// SPDX-License-Identifier: Apache-2.0
//! The schema cache behind the catalog commands (M1-16, docs/10 §1): cached
//! reads with persisted snapshots, and invalidation after DDL.
use std::sync::Arc;

use openrows_driver_api::{BoxFuture, Catalog, DriverError};
use openrows_schema::{Cached, DatabaseKey, Listing, SchemaCache, Scope, save_snapshot};
use openrows_sql::StatementKind;
use openrows_store::LocalStore;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::Connections;
use super::catalog::read_catalog;
use super::catalog_dto::{CatalogReply, ObjectDto, ObjectsReply};
use super::error::{IpcError, IpcErrorKind};
use crate::queries::history::now_ms;

/// Tells the webview that DDL ran on a connection: what it shows of that
/// connection's catalog is stale and should be read again.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogChanged {
    profile_id: String,
    /// The database the DDL ran on.
    database: String,
}

impl Event for CatalogChanged {
    const NAME: &'static str = "catalog-changed";
}

/// One listing to serve: from the cache unless `fresh` (or not cached yet).
pub(crate) struct CatalogRead {
    pub(crate) key: DatabaseKey,
    pub(crate) scope: Scope,
    pub(crate) fresh: bool,
}

impl CatalogRead {
    pub(crate) fn new((profile_id, database): (String, String), scope: Scope, fresh: bool) -> Self {
        Self {
            key: DatabaseKey {
                profile_id,
                database,
            },
            scope,
            fresh,
        }
    }
}

/// Serve `read` from the cache, or run `fetch` on the database's metadata
/// session, keep the result and persist it as the scope's snapshot.
pub(crate) async fn cached_read(
    app: &AppHandle,
    read: CatalogRead,
    fetch: impl for<'s> FnOnce(&'s mut dyn Catalog) -> BoxFuture<'s, Result<Listing, DriverError>>,
) -> Result<Cached, IpcError> {
    let cache = app.state::<SchemaCache>();
    if !read.fresh
        && let Some(hit) = cache.get(&app.state::<LocalStore>(), &read.key, &read.scope)
    {
        return Ok(hit);
    }
    let token = cache.begin(&read.key);
    let connections = app.state::<Connections>();
    let target = (read.key.profile_id.as_str(), read.key.database.as_str());
    let listing = read_catalog(&connections, target, fetch).await?;
    let cached = cache.put(&read.key, &read.scope, listing, token);
    persist(app, read, Arc::clone(&cached.listing));
    Ok(cached)
}

/// Write the snapshot off the async runtime; the live listing is already
/// served, so a failed write only costs the next launch its instant tree.
fn persist(app: &AppHandle, read: CatalogRead, listing: Arc<Listing>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<LocalStore>();
        let _ = save_snapshot(&store, (&read.key, &read.scope), &listing, now_ms());
    });
}

/// Map a cached listing's items into a reply.
pub(crate) fn reply<'a, S: 'a, D>(
    cached: &'a Cached,
    items: impl Fn(&'a Listing) -> Option<&'a [S]>,
    map: impl Fn(&'a S) -> D,
) -> Result<CatalogReply<Vec<D>>, IpcError> {
    let items = items(&cached.listing).ok_or_else(mismatch)?;
    Ok(CatalogReply {
        data: items.iter().map(map).collect(),
        stale: cached.stale,
    })
}

/// The first `limit` objects of a cached section (all without a limit) and
/// how many it holds.
pub(crate) fn page(cached: &Cached, limit: Option<u32>) -> Result<ObjectsReply, IpcError> {
    let objects = cached.listing.objects().ok_or_else(mismatch)?;
    let shown = limit.map_or(objects.len(), |limit| {
        usize::try_from(limit).unwrap_or(usize::MAX)
    });
    Ok(ObjectsReply {
        data: objects.iter().take(shown).map(ObjectDto::from).collect(),
        total: u32::try_from(objects.len()).unwrap_or(u32::MAX),
        stale: cached.stale,
    })
}

/// A scope always caches the listing its command stored.
pub(crate) fn mismatch() -> IpcError {
    IpcError::new(
        IpcErrorKind::Internal,
        "The schema cache holds another listing for this scope",
    )
}

/// After statements ran on `database`: when one of them is DDL, the
/// connection's cached catalog is stale and the webview refreshes its tree.
pub(crate) fn after_statements(
    app: &AppHandle,
    (profile_id, database): (&str, &str),
    kinds: impl IntoIterator<Item = StatementKind>,
) {
    if changes_schema(kinds) {
        app.state::<SchemaCache>().invalidate(profile_id);
        // Emitting only fails while the webview is gone; it reads afresh on reload.
        let _ = CatalogChanged {
            profile_id: profile_id.to_owned(),
            database: database.to_owned(),
        }
        .emit(app);
    }
}

fn changes_schema(kinds: impl IntoIterator<Item = StatementKind>) -> bool {
    kinds.into_iter().any(|kind| kind == StatementKind::Ddl)
}

#[cfg(test)]
mod tests {
    use openrows_driver_api::catalog::SchemaSummary;

    use super::*;
    use crate::connections::catalog_dto::SchemaDto;

    #[test]
    fn only_ddl_changes_the_schema() {
        use StatementKind::{Ddl, Read, Unknown, Write};
        assert!(changes_schema([Read, Ddl]));
        assert!(!changes_schema([Read, Write, Unknown]));
        assert!(!changes_schema([]));
    }

    fn cached(listing: Listing) -> Cached {
        let cache = SchemaCache::default();
        let key = DatabaseKey {
            profile_id: "p".into(),
            database: "d".into(),
        };
        let token = cache.begin(&key);
        cache.put(&key, &Scope::Schemas, listing, token)
    }

    #[test]
    fn object_pages_hold_the_first_objects_and_the_total() -> Result<(), IpcError> {
        use openrows_driver_api::catalog::{ObjectKind, ObjectSummary};
        let object = |name: &str| ObjectSummary {
            schema: "public".into(),
            name: name.into(),
            kind: ObjectKind::Table,
            arguments: None,
            row_estimate: None,
            comment: None,
        };
        let listing = Listing::Objects(vec![object("a"), object("b"), object("c")]);
        let section = cached(listing);
        let first = page(&section, Some(2))?;
        assert_eq!((first.data.len(), first.total), (2, 3));
        assert_eq!(page(&section, None)?.data.len(), 3);
        assert_eq!(page(&section, Some(10))?.data.len(), 3);
        Ok(())
    }

    #[test]
    fn replies_map_the_scope_listing_and_reject_another() -> Result<(), IpcError> {
        let schemas = cached(Listing::Schemas(vec![SchemaSummary {
            name: "public".into(),
            system: false,
            comment: None,
        }]));
        let reply = reply(&schemas, Listing::schemas, SchemaDto::from)?;
        assert_eq!((reply.data.len(), reply.stale), (1, false));
        let error = super::reply(&schemas, Listing::databases, |_| ()).err();
        assert_eq!(error.map(|error| error.kind), Some(IpcErrorKind::Internal));
        Ok(())
    }
}
