// SPDX-License-Identifier: Apache-2.0
//! Agent-app credential links and per-credential usage, beside the token table.
use crate::{
    McpError, TokenStore,
    activity::{ActivityEvent, ActivityLog, TokenUsage},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::sync::PoisonError;

/// An agent app registered from the Apps page and the credential it uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppLink {
    /// Random client id passed to the bridge as `--client`; names its keychain item.
    pub client: String,
    /// Agent app (`claude`, `codex`, `gemini`).
    pub app: String,
    /// Current credential id.
    pub token: String,
}

pub(crate) fn migrate(database: &Connection) -> Result<(), McpError> {
    database
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS links(client TEXT PRIMARY KEY, app TEXT UNIQUE NOT NULL, token TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS usage(token TEXT PRIMARY KEY, calls INTEGER NOT NULL, seen INTEGER NOT NULL, client TEXT);",
        )
        .map_err(|_| McpError::Store)
}
/// Remove a revoked credential's counters; its link stays until unlinked.
pub(crate) fn forget(database: &Connection, token: &str) -> Result<(), McpError> {
    database
        .execute("DELETE FROM usage WHERE token=?1", [token])
        .map(drop)
        .map_err(|_| McpError::Store)
}
impl TokenStore {
    /// Live and recent external tool calls.
    #[must_use]
    pub fn activity(&self) -> &ActivityLog {
        &self.activity
    }
    /// Record (or replace) the credential an app's client id uses.
    /// # Errors
    /// Invalid identifiers or local storage failure.
    pub fn link(&self, link: &AppLink) -> Result<(), McpError> {
        for id in [&link.client, &link.app, &link.token] {
            openrows_ai::validate_id(id).map_err(|_| McpError::Invalid)?;
        }
        self.database
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execute(
                "INSERT OR REPLACE INTO links VALUES(?1,?2,?3)",
                params![link.client, link.app, link.token],
            )
            .map(drop)
            .map_err(|_| McpError::Store)
    }
    /// Every registered app link.
    /// # Errors
    /// Local storage failure.
    pub fn links(&self) -> Result<Vec<AppLink>, McpError> {
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let mut query = database
            .prepare("SELECT client,app,token FROM links ORDER BY app")
            .map_err(|_| McpError::Store)?;
        query
            .query_map([], |row| {
                Ok(AppLink {
                    client: row.get(0)?,
                    app: row.get(1)?,
                    token: row.get(2)?,
                })
            })
            .map_err(|_| McpError::Store)?
            .map(|row| row.map_err(|_| McpError::Store))
            .collect()
    }
    /// Forget an app's link; revoke its credential separately.
    /// # Errors
    /// Local storage failure.
    pub fn unlink(&self, client: &str) -> Result<(), McpError> {
        self.database
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execute("DELETE FROM links WHERE client=?1", [client])
            .map(drop)
            .map_err(|_| McpError::Store)
    }
    /// Per-credential counters.
    /// # Errors
    /// Local storage failure.
    pub fn usage(&self) -> Result<Vec<TokenUsage>, McpError> {
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let mut query = database
            .prepare("SELECT token,calls,seen,client FROM usage ORDER BY token")
            .map_err(|_| McpError::Store)?;
        query
            .query_map([], |row| {
                Ok(TokenUsage {
                    token: row.get(0)?,
                    calls: row.get(1)?,
                    seen: row.get(2)?,
                    client: row.get(3)?,
                })
            })
            .map_err(|_| McpError::Store)?
            .map(|row| row.map_err(|_| McpError::Store))
            .collect()
    }
    /// The app linked to a credential, if any.
    pub(crate) fn app_of(&self, token: &str) -> Option<String> {
        self.database
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .query_row("SELECT app FROM links WHERE token=?1", [token], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }
    /// Count a tool call, update last-seen and publish the event.
    pub(crate) fn record(&self, event: ActivityEvent, client: Option<&str>) {
        self.touch(&event.token, 1, event.at, client);
        self.activity.push(event);
    }
    /// Update last-seen for a request that is not a tool call.
    pub(crate) fn seen(&self, token: &str, client: Option<&str>) {
        if let Ok(now) = crate::tokens::now() {
            self.touch(token, 0, now, client);
        }
    }
    fn touch(&self, token: &str, calls: u32, at: u32, client: Option<&str>) {
        // Counters are advisory; a storage failure must not fail the tool call.
        let _ = self
            .database
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execute(
                "INSERT INTO usage VALUES(?1,?2,?3,?4) ON CONFLICT(token) DO UPDATE SET calls=calls+excluded.calls, seen=excluded.seen, client=COALESCE(excluded.client, client)",
                params![token, calls, at, client],
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TokenScope, activity::ActivityOutcome};

    fn scope() -> TokenScope {
        TokenScope {
            provider: "provider".into(),
            connections: vec!["connection".into()],
            expires: crate::tokens::now().unwrap_or_default() + 60,
        }
    }

    #[test]
    fn links_usage_and_revocation_round_trip() -> Result<(), McpError> {
        let store = TokenStore::memory()?;
        let token = store.create(scope())?.info.id;
        let link = AppLink {
            client: "claude-0a1b".into(),
            app: "claude".into(),
            token: token.clone(),
        };
        store.link(&link)?;
        assert_eq!(store.links()?, vec![link]);
        assert_eq!(store.app_of(&token).as_deref(), Some("claude"));
        let event = ActivityEvent {
            at: 7,
            token: token.clone(),
            app: Some("claude".into()),
            tool: "list_schemas".into(),
            connection: Some("connection".into()),
            outcome: ActivityOutcome::Ok,
        };
        store.record(event.clone(), Some("claude-code 2"));
        store.record(event, None);
        store.seen(&token, None);
        let usage = store.usage()?;
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].calls, 2);
        assert_eq!(usage[0].client.as_deref(), Some("claude-code 2"));
        assert_eq!(store.activity().recent(Some(&token)).len(), 2);
        store.revoke(&token)?;
        assert!(store.usage()?.is_empty());
        assert!(store.activity().recent(Some(&token)).is_empty());
        store.unlink("claude-0a1b")?;
        assert!(store.links()?.is_empty());
        Ok(())
    }

    #[test]
    fn links_refuse_unsafe_identifiers() -> Result<(), McpError> {
        let store = TokenStore::memory()?;
        let link = AppLink {
            client: "../x".into(),
            app: "claude".into(),
            token: "t".into(),
        };
        assert_eq!(store.link(&link), Err(McpError::Invalid));
        Ok(())
    }
}
