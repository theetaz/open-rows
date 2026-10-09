// SPDX-License-Identifier: Apache-2.0
//! Session and workspace state of each window (docs/15 §2, M1-21, M1-26):
//! the webview's JSON for one part (open query tabs, open connections),
//! written debounced while the user works and read once on launch.

use rusqlite::{OptionalExtension, params};

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// Where one piece of workspace state belongs: a window and a part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceKey<'a> {
    /// The window, e.g. `main`.
    pub window: &'a str,
    /// The part of the window's state, e.g. `queryTabs` or `session`.
    pub part: &'a str,
}

const UPSERT: &str = "INSERT INTO workspace_state (window_id, part, state, updated_at) \
     VALUES (?1, ?2, ?3, ?4) ON CONFLICT (window_id, part) \
     DO UPDATE SET state = excluded.state, updated_at = excluded.updated_at";

impl LocalStore {
    /// The stored state of `key`, or `None` when nothing was saved.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn workspace_state(&self, key: WorkspaceKey<'_>) -> Result<Option<String>, StoreError> {
        self.lock()?
            .query_row(
                "SELECT state FROM workspace_state WHERE window_id = ?1 AND part = ?2",
                [key.window, key.part],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite("read workspace state"))
    }

    /// Insert or replace the state of `key`, written at `now_ms`.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn save_workspace_state(
        &self,
        key: WorkspaceKey<'_>,
        state: &str,
        now_ms: i64,
    ) -> Result<(), StoreError> {
        self.lock()?
            .execute(UPSERT, params![key.window, key.part, state, now_ms])
            .map(drop)
            .map_err(sqlite("write workspace state"))
    }

    /// Forget the state of `key`; a no-op when none is stored.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn clear_workspace_state(&self, key: WorkspaceKey<'_>) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                "DELETE FROM workspace_state WHERE window_id = ?1 AND part = ?2",
                [key.window, key.part],
            )
            .map(drop)
            .map_err(sqlite("clear workspace state"))
    }
}
