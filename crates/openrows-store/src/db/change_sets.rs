// SPDX-License-Identifier: Apache-2.0
//! The journal of staged table edits (docs/15 §2 `change_sets`, NFR-REL-01).
//!
//! The webview owns the change-set format; the store keeps it as an opaque
//! JSON document keyed by table so it can be offered back after a crash.

use rusqlite::{OptionalExtension, params};

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// A journaled change set; `ops` is the webview's JSON payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSetRecord {
    /// Stable key of the edited table, e.g. the JSON of its target.
    pub id: String,
    /// Connection profile the edits belong to.
    pub profile_id: String,
    /// JSON description of the edited table.
    pub target: String,
    /// Staged edits with their exact originals, as JSON.
    pub ops: String,
    /// Last write, Unix epoch milliseconds.
    pub updated_at_ms: i64,
}

const UPSERT: &str = "INSERT INTO change_sets (id, profile_id, target, ops, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5) \
     ON CONFLICT (id) DO UPDATE SET profile_id = excluded.profile_id, \
     target = excluded.target, ops = excluded.ops, updated_at = excluded.updated_at";

const SELECT: &str =
    "SELECT id, profile_id, target, ops, updated_at FROM change_sets WHERE id = ?1";

const SELECT_ALL: &str = "SELECT id, profile_id, target, ops, updated_at FROM change_sets \
     ORDER BY updated_at DESC";

fn record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChangeSetRecord> {
    Ok(ChangeSetRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        target: row.get(2)?,
        ops: row.get(3)?,
        updated_at_ms: row.get(4)?,
    })
}

impl LocalStore {
    /// Insert or replace the journaled change set of one table.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn save_change_set(&self, record: &ChangeSetRecord) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                UPSERT,
                params![
                    record.id,
                    record.profile_id,
                    record.target,
                    record.ops,
                    record.updated_at_ms,
                ],
            )
            .map(drop)
            .map_err(sqlite("save change set"))
    }

    /// The journaled change set of one table, if any.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn change_set(&self, id: &str) -> Result<Option<ChangeSetRecord>, StoreError> {
        self.lock()?
            .query_row(SELECT, [id], record)
            .optional()
            .map_err(sqlite("read change set"))
    }

    /// Every journaled change set, newest first.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn change_sets(&self) -> Result<Vec<ChangeSetRecord>, StoreError> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(SELECT_ALL)
            .map_err(sqlite("list change sets"))?;
        let rows = statement
            .query_map([], record)
            .map_err(sqlite("list change sets"))?;
        rows.collect::<Result<_, _>>()
            .map_err(sqlite("list change sets"))
    }

    /// Forget the journaled change set of one table (after commit or discard).
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn clear_change_set(&self, id: &str) -> Result<(), StoreError> {
        self.lock()?
            .execute("DELETE FROM change_sets WHERE id = ?1", [id])
            .map(drop)
            .map_err(sqlite("clear change set"))
    }
}
