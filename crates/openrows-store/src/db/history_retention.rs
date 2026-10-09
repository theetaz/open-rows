// SPDX-License-Identifier: Apache-2.0
//! Count and age limits applied transactionally to history.
use super::{LocalStore, StoreError, error::sqlite};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
/// Device-local retention; keep forever bypasses both limits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryRetention {
    /// Maximum entries per connection (at least one).
    pub max_entries: u32,
    /// Maximum age in days (at least one).
    pub max_age_days: u32,
    /// Explicitly retain every history entry.
    pub keep_forever: bool,
}
impl Default for HistoryRetention {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_age_days: 90,
            keep_forever: false,
        }
    }
}
/// Prune inside the writer transaction; row ids break timestamp ties.
pub(super) fn prune(
    conn: &Connection,
    policy: HistoryRetention,
    now_ms: i64,
) -> Result<usize, StoreError> {
    if policy.keep_forever {
        return Ok(0);
    }
    let before = now_ms.saturating_sub(i64::from(policy.max_age_days.max(1)) * 86_400_000);
    let age = conn
        .execute("DELETE FROM history WHERE started_at < ?1", [before])
        .map_err(sqlite("prune history age"))?;
    let count = conn.execute("DELETE FROM history WHERE id IN (SELECT id FROM (SELECT id, row_number() OVER (PARTITION BY profile_id ORDER BY started_at DESC, id DESC) AS position FROM history) WHERE position > ?1)", params![policy.max_entries.max(1)]).map_err(sqlite("prune history count"))?;
    Ok(age + count)
}
/// Write-time pruning only scans the connection that received a statement.
pub(super) fn prune_profile(
    conn: &Connection,
    policy: HistoryRetention,
    now_ms: i64,
    profile: &str,
) -> Result<(), StoreError> {
    if policy.keep_forever {
        return Ok(());
    }
    let before = now_ms.saturating_sub(i64::from(policy.max_age_days.max(1)) * 86_400_000);
    conn.execute(
        "DELETE FROM history WHERE profile_id = ?1 AND started_at < ?2",
        params![profile, before],
    )
    .map_err(sqlite("prune history age"))?;
    conn.execute("DELETE FROM history WHERE id IN (SELECT id FROM history WHERE profile_id = ?1 ORDER BY started_at DESC, id DESC LIMIT -1 OFFSET ?2)", params![profile, policy.max_entries.max(1)]).map_err(sqlite("prune history count"))?;
    Ok(())
}

impl LocalStore {
    /// Install retention and sweep existing rows on startup or settings changes.
    /// # Errors
    /// Returns a storage error if the sweep cannot finish.
    pub fn set_history_retention(
        &self,
        policy: HistoryRetention,
        now_ms: i64,
    ) -> Result<usize, StoreError> {
        let mut current = self
            .history_retention
            .lock()
            .map_err(|_| StoreError::Poisoned)?;
        let mut conn = self.lock()?;
        let transaction = conn
            .transaction()
            .map_err(sqlite("begin history retention"))?;
        let removed = prune(&transaction, policy, now_ms)?;
        transaction
            .commit()
            .map_err(sqlite("commit history retention"))?;
        *current = policy;
        Ok(removed)
    }
    /// Remove all history, including profiles no longer saved.
    /// # Errors
    /// Returns a storage error if deletion fails.
    pub fn clear_all_history(&self) -> Result<usize, StoreError> {
        self.lock()?
            .execute("DELETE FROM history", [])
            .map_err(sqlite("clear all history"))
    }
}
