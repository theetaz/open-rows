// SPDX-License-Identifier: Apache-2.0
//! Persisted schema-cache snapshots (docs/15 §2, M1-16): one opaque payload
//! per listed scope of a connection's database, written after every live
//! catalog read and loaded on the first read after a relaunch.

use rusqlite::{Row, params};

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// One persisted scope; the payload format belongs to `openrows-schema`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaSnapshotRecord {
    /// Connection profile the listing was read on.
    pub profile_id: String,
    /// Database the listing belongs to.
    pub database: String,
    /// Stable scope key, e.g. `schemas` or `objects:table,view:public`.
    pub scope: String,
    /// Encoded listing.
    pub payload: Vec<u8>,
    /// When it was read from the server, Unix epoch milliseconds.
    pub fetched_at_ms: i64,
}

const UPSERT: &str = "INSERT INTO schema_snapshots \
     (profile_id, database_name, scope, payload, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5) \
     ON CONFLICT (profile_id, database_name, scope) \
     DO UPDATE SET payload = excluded.payload, fetched_at = excluded.fetched_at";

const SELECT: &str = "SELECT profile_id, database_name, scope, payload, fetched_at \
     FROM schema_snapshots WHERE profile_id = ?1 AND database_name = ?2";

fn record(row: &Row<'_>) -> rusqlite::Result<SchemaSnapshotRecord> {
    Ok(SchemaSnapshotRecord {
        profile_id: row.get(0)?,
        database: row.get(1)?,
        scope: row.get(2)?,
        payload: row.get(3)?,
        fetched_at_ms: row.get(4)?,
    })
}

impl LocalStore {
    /// Insert or replace one scope's snapshot.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn save_schema_snapshot(&self, snapshot: &SchemaSnapshotRecord) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                UPSERT,
                params![
                    snapshot.profile_id,
                    snapshot.database,
                    snapshot.scope,
                    snapshot.payload,
                    snapshot.fetched_at_ms
                ],
            )
            .map(drop)
            .map_err(sqlite("save schema snapshot"))
    }

    /// Every persisted scope of one connection database.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn schema_snapshots(
        &self,
        profile_id: &str,
        database: &str,
    ) -> Result<Vec<SchemaSnapshotRecord>, StoreError> {
        let conn = self.lock()?;
        let mut statement = conn
            .prepare(SELECT)
            .map_err(sqlite("read schema snapshots"))?;
        let rows = statement
            .query_map([profile_id, database], record)
            .map_err(sqlite("read schema snapshots"))?;
        rows.collect::<Result<_, _>>()
            .map_err(sqlite("read schema snapshots"))
    }

    /// Delete every snapshot of a connection (it was deleted); returns the count.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn delete_schema_snapshots(&self, profile_id: &str) -> Result<usize, StoreError> {
        self.lock()?
            .execute(
                "DELETE FROM schema_snapshots WHERE profile_id = ?1",
                [profile_id],
            )
            .map_err(sqlite("delete schema snapshots"))
    }

    /// Delete snapshots last read before `before_ms` (unused connections);
    /// returns the count.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn prune_schema_snapshots(&self, before_ms: i64) -> Result<usize, StoreError> {
        self.lock()?
            .execute(
                "DELETE FROM schema_snapshots WHERE fetched_at < ?1",
                [before_ms],
            )
            .map_err(sqlite("prune schema snapshots"))
    }
}
