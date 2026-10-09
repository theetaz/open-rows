// SPDX-License-Identifier: Apache-2.0
//! Schema-tree commands (D-06, M1-16): databases, schemas, objects and the
//! name index, served by the schema cache ([`super::catalog_cache`]) and read
//! on the connection's per-database metadata sessions (closed by
//! `session_close` and `connection_close`) when missing or `fresh`.
use openrows_driver_api::catalog::{ColumnSummary, ObjectKind, TableStructure};
use openrows_driver_api::diagram::SchemaDiagram;
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{BoxFuture, Catalog, DriverError, catalog};
use openrows_schema::{DatabaseKey, Listing, NameIndex, SchemaCache, Scope, SearchQuery};
use openrows_sql::StatementKind;
use tauri::{AppHandle, Manager, State};

use super::Connections;
use super::catalog_cache::{CatalogRead, after_statements, cached_read, mismatch, page, reply};
use super::catalog_dto::{
    CatalogReply, CatalogSearchReply, CatalogSearchRequest, DatabaseDto, ObjectDto, ObjectsReply,
    ObjectsRequest, SchemaDto,
};
use super::error::IpcError;
use super::route::profile_target;

/// Run `read` on the catalog facet of `database`'s metadata session.
pub(super) async fn read_catalog<T>(
    state: &Connections,
    (profile_id, database): (&str, &str),
    read: impl for<'s> FnOnce(&'s mut dyn Catalog) -> BoxFuture<'s, Result<T, DriverError>>,
) -> Result<T, IpcError> {
    state
        .metadata
        .read(
            (profile_id, database),
            || profile_target(state, profile_id),
            move |connection| match catalog(connection) {
                Ok(facet) => read(facet),
                Err(error) => Box::pin(async move { Err(error) }),
            },
        )
        .await
}

/// Databases on the server, read through `database`'s metadata session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_databases(
    app: AppHandle,
    profile_id: String,
    database: String,
    fresh: bool,
) -> Result<CatalogReply<Vec<DatabaseDto>>, IpcError> {
    let read = CatalogRead::new((profile_id, database), Scope::Databases, fresh);
    let cached = cached_read(&app, read, |catalog| {
        Box::pin(async move { catalog.list_databases().await.map(Listing::Databases) })
    })
    .await?;
    reply(&cached, Listing::databases, DatabaseDto::from)
}

/// Schemas of `database`, user schemas first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_schemas(
    app: AppHandle,
    profile_id: String,
    database: String,
    fresh: bool,
) -> Result<CatalogReply<Vec<SchemaDto>>, IpcError> {
    let read = CatalogRead::new((profile_id, database), Scope::Schemas, fresh);
    let cached = cached_read(&app, read, |catalog| {
        Box::pin(async move { catalog.list_schemas().await.map(Listing::Schemas) })
    })
    .await?;
    reply(&cached, Listing::schemas, SchemaDto::from)
}

/// Objects of some kinds in one schema of `database`, ordered by name: the
/// first `limit` of them and the total, so large schemas list in pages.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_objects(
    app: AppHandle,
    profile_id: String,
    database: String,
    request: ObjectsRequest,
) -> Result<ObjectsReply, IpcError> {
    let kinds: Vec<ObjectKind> = request.kinds.into_iter().map(ObjectKind::from).collect();
    let scope = Scope::objects(&request.schema, &kinds);
    let read = CatalogRead::new((profile_id, database), scope, request.fresh);
    let schema = request.schema;
    let cached = cached_read(&app, read, move |catalog| {
        Box::pin(async move {
            let objects = catalog.list_objects(&schema, &kinds).await?;
            Ok(Listing::Objects(objects))
        })
    })
    .await?;
    page(&cached, request.limit)
}

/// On window focus: compare the server's schema fingerprint with the one
/// seen before; when DDL ran outside the app, mark the connection's cache
/// stale and tell the webview (`catalog-changed`), which reads it again.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_check(
    app: AppHandle,
    profile_id: String,
    database: String,
) -> Result<bool, IpcError> {
    let connections = app.state::<Connections>();
    let target = (profile_id.as_str(), database.as_str());
    let fingerprint =
        read_catalog(&connections, target, |catalog| catalog.schema_fingerprint()).await?;
    let Some(fingerprint) = fingerprint else {
        return Ok(false);
    };
    let key = DatabaseKey {
        profile_id: profile_id.clone(),
        database: database.clone(),
    };
    let changed = app
        .state::<SchemaCache>()
        .observe_fingerprint(&key, fingerprint);
    if changed {
        after_statements(&app, target, [StatementKind::Ddl]);
    }
    Ok(changed)
}

/// Search the bounded name index of `database` (tables, views, sequences,
/// routines of every user schema), reading the index first when needed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_search(
    app: AppHandle,
    profile_id: String,
    database: String,
    request: CatalogSearchRequest,
) -> Result<CatalogSearchReply, IpcError> {
    let read = CatalogRead::new((profile_id, database), Scope::Index, request.fresh);
    let cached = cached_read(&app, read, |catalog| {
        Box::pin(async move {
            let objects = catalog.search_index().await?;
            Ok(Listing::Index(NameIndex::new(objects)))
        })
    })
    .await?;
    let index = cached.listing.index().ok_or_else(mismatch)?;
    let hits = index.search(&SearchQuery {
        text: &request.query,
        schema: request.schema.as_deref(),
        limit: request.limit.map_or(usize::MAX, |limit| {
            usize::try_from(limit).unwrap_or(usize::MAX)
        }),
    });
    Ok(CatalogSearchReply {
        objects: hits.objects.into_iter().map(ObjectDto::from).collect(),
        total: u32::try_from(hits.total).unwrap_or(u32::MAX),
        stale: cached.stale,
    })
}

/// Mark everything cached for a connection stale (⌘R); the next reads go
/// to the server.
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
pub(crate) fn catalog_invalidate(cache: State<'_, SchemaCache>, profile_id: String) {
    cache.invalidate(&profile_id);
}

/// A relation's columns, indexes and row estimate on `database`'s metadata
/// session (the D-08 table view's Structure and Indexes tabs).
pub(crate) async fn read_structure(
    state: &Connections,
    (profile_id, database): (&str, &str),
    (schema, name): (String, String),
) -> Result<TableStructure, IpcError> {
    read_catalog(state, (profile_id, database), move |catalog| {
        Box::pin(async move { catalog.table_structure(&schema, &name).await })
    })
    .await
}

/// A relation's catalog columns on `database`'s metadata session; table
/// browsing validates every requested identifier against them.
pub(crate) async fn read_columns(
    state: &Connections,
    (profile_id, database): (&str, &str),
    (schema, name): (String, String),
) -> Result<Vec<ColumnSummary>, IpcError> {
    read_catalog(state, (profile_id, database), move |catalog| {
        Box::pin(async move { catalog.table_columns(&schema, &name).await })
    })
    .await
}

/// A relation's columns, constraints, indexes, triggers, sizes and comment on
/// `database`'s metadata session (Structure, Index, Triggers and DDL views).
pub(crate) async fn read_detail(
    state: &Connections,
    (profile_id, database): (&str, &str),
    (schema, name): (String, String),
) -> Result<TableDetail, IpcError> {
    read_catalog(state, (profile_id, database), move |catalog| {
        Box::pin(async move { catalog.table_detail(&schema, &name).await })
    })
    .await
}

/// Tables, columns and foreign keys of `schema` (at most `max_tables`) on
/// `database`'s metadata session, for the ER diagram.
pub(crate) async fn read_diagram(
    state: &Connections,
    (profile_id, database): (&str, &str),
    schema: String,
    max_tables: usize,
) -> Result<SchemaDiagram, IpcError> {
    read_catalog(state, (profile_id, database), move |catalog| {
        Box::pin(async move { catalog.schema_diagram(&schema, max_tables).await })
    })
    .await
}
