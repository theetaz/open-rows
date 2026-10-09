// SPDX-License-Identifier: Apache-2.0
//! In-memory listings per connection database, backed by persisted
//! snapshots and invalidated by reconnects and DDL.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use openrows_store::{LocalStore, SchemaSnapshotRecord, StoreError};

use crate::codec;
use crate::listing::Listing;
use crate::scope::Scope;

/// A connection profile's database.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DatabaseKey {
    /// Connection profile id.
    pub profile_id: String,
    /// Database name.
    pub database: String,
}

/// A cached listing and whether it still needs a live read.
#[derive(Clone, Debug)]
pub struct Cached {
    /// The listing.
    pub listing: Arc<Listing>,
    /// From a snapshot, or invalidated since it was read: show it, then
    /// read it again in the background.
    pub stale: bool,
}

/// Taken before a live read; a read that overlapped an invalidation is
/// stored stale, so it is read again.
#[derive(Clone, Copy, Debug)]
pub struct ReadToken(u64);

#[derive(Default)]
struct DatabaseEntry {
    scopes: HashMap<String, Cached>,
    /// Bumped by every invalidation.
    epoch: u64,
    /// Persisted snapshots were merged in.
    loaded: bool,
}

/// Listings of every browsed connection database (docs/10 §1).
#[derive(Default)]
pub struct SchemaCache {
    entries: Mutex<HashMap<DatabaseKey, DatabaseEntry>>,
}

impl SchemaCache {
    /// The cached listing of `scope`, loading the database's persisted
    /// snapshots (stale) on its first use. An unreadable store or payload
    /// only means there is no snapshot: the caller reads live.
    pub fn get(&self, store: &LocalStore, key: &DatabaseKey, scope: &Scope) -> Option<Cached> {
        let mut entries = self.lock();
        let entry = entries.entry(key.clone()).or_default();
        if !entry.loaded {
            entry.loaded = true;
            merge_snapshots(
                entry,
                store.schema_snapshots(&key.profile_id, &key.database),
            );
        }
        entry.scopes.get(&scope.key()).cloned()
    }

    /// Start a live read of `key`.
    pub fn begin(&self, key: &DatabaseKey) -> ReadToken {
        ReadToken(self.lock().get(key).map_or(0, |entry| entry.epoch))
    }

    /// Keep a live read; it is fresh unless `key` was invalidated since
    /// [`SchemaCache::begin`].
    pub fn put(
        &self,
        key: &DatabaseKey,
        scope: &Scope,
        listing: Listing,
        token: ReadToken,
    ) -> Cached {
        let mut entries = self.lock();
        let entry = entries.entry(key.clone()).or_default();
        let cached = Cached {
            listing: Arc::new(listing),
            stale: entry.epoch != token.0,
        };
        entry.scopes.insert(scope.key(), cached.clone());
        cached
    }

    /// Mark every listing of a connection stale (it reconnected or ran DDL).
    pub fn invalidate(&self, profile_id: &str) {
        let mut entries = self.lock();
        let connection = entries
            .iter_mut()
            .filter(|(key, _)| key.profile_id == profile_id);
        for (_, entry) in connection {
            entry.epoch += 1;
            entry
                .scopes
                .values_mut()
                .for_each(|cached| cached.stale = true);
        }
    }

    /// Drop a deleted connection's listings and snapshots.
    ///
    /// # Errors
    /// The snapshots could not be deleted from the local store.
    pub fn forget(&self, store: &LocalStore, profile_id: &str) -> Result<(), StoreError> {
        self.lock().retain(|key, _| key.profile_id != profile_id);
        store.delete_schema_snapshots(profile_id).map(drop)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<DatabaseKey, DatabaseEntry>> {
        // A panic mid-update leaves at worst a stale listing, never a torn one.
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Persist one live read as the scope's snapshot.
///
/// # Errors
/// The listing could not be encoded or the store write failed.
pub fn save_snapshot(
    store: &LocalStore,
    (key, scope): (&DatabaseKey, &Scope),
    listing: &Listing,
    fetched_at_ms: i64,
) -> Result<(), SnapshotError> {
    let payload = codec::encode(listing).map_err(SnapshotError::Encode)?;
    store
        .save_schema_snapshot(&SchemaSnapshotRecord {
            profile_id: key.profile_id.clone(),
            database: key.database.clone(),
            scope: scope.key(),
            payload,
            fetched_at_ms,
        })
        .map_err(SnapshotError::Store)
}

/// Why a snapshot was not written.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// The listing could not be encoded.
    #[error("schema snapshot could not be encoded")]
    Encode(#[source] serde_json::Error),
    /// The local store refused the write.
    #[error(transparent)]
    Store(StoreError),
}

/// Add persisted scopes that no live read has filled yet, all stale.
fn merge_snapshots(entry: &mut DatabaseEntry, rows: Result<Vec<SchemaSnapshotRecord>, StoreError>) {
    for row in rows.into_iter().flatten() {
        if entry.scopes.contains_key(&row.scope) {
            continue;
        }
        if let Some(listing) = codec::decode(&row.payload) {
            let cached = Cached {
                listing: Arc::new(listing),
                stale: true,
            };
            entry.scopes.insert(row.scope, cached);
        }
    }
}
