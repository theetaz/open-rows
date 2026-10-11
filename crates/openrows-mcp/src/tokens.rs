// SPDX-License-Identifier: Apache-2.0
//! Private hashed credentials and live cancellation, independent of settings JSON.
use crate::{ClientInfo, CreatedToken, McpError, TokenInfo, TokenScope};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

/// Clones share local persistence and connected-client cancellation.
#[derive(Clone)]
pub struct TokenStore {
    database: Arc<Mutex<Connection>>,
    clients: Arc<Mutex<HashMap<String, Client>>>,
}
struct Client {
    cancel: Arc<CancellationToken>,
    active: u32,
}
/// An authenticated request whose Drop decrements bounded activity.
pub(crate) struct Grant {
    pub(crate) info: TokenInfo,
    pub(crate) cancel: Arc<CancellationToken>,
    store: TokenStore,
}
impl Drop for Grant {
    fn drop(&mut self) {
        if let Some(client) = self
            .store
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(&self.info.id)
            .filter(|client| Arc::ptr_eq(&client.cancel, &self.cancel))
        {
            client.active = client.active.saturating_sub(1);
        }
    }
}
impl TokenStore {
    /// Create a private local credential store; values are never persisted.
    /// # Errors
    /// Fixed storage error, without paths or database diagnostics.
    pub fn open(path: &Path) -> Result<Self, McpError> {
        let database = Connection::open(path).map_err(|_| McpError::Store)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| McpError::Store)?;
        }
        Self::new(database)
    }
    /// Isolated test/embedded storage.
    /// # Errors
    /// Local `SQLite` initialization failure.
    pub fn memory() -> Result<Self, McpError> {
        Self::new(Connection::open_in_memory().map_err(|_| McpError::Store)?)
    }
    fn new(database: Connection) -> Result<Self, McpError> {
        database.execute_batch("CREATE TABLE IF NOT EXISTS tokens(id TEXT PRIMARY KEY, hash BLOB UNIQUE NOT NULL, scope TEXT NOT NULL)").map_err(|_| McpError::Store)?;
        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            clients: Arc::default(),
        })
    }
    /// Create a cryptographically random token, returned once with its metadata.
    /// # Errors
    /// Invalid scope, expiry, quota or local storage failure.
    pub fn create(&self, scope: TokenScope) -> Result<CreatedToken, McpError> {
        validate(&scope)?;
        let value = random_id();
        let info = TokenInfo {
            id: random_id(),
            scope,
        };
        let scope = serde_json::to_string(&info.scope).map_err(|_| McpError::Invalid)?;
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let count: u32 = database
            .query_row("SELECT COUNT(*) FROM tokens", [], |row| row.get(0))
            .map_err(|_| McpError::Store)?;
        if count >= 32 {
            return Err(McpError::Limit);
        }
        database
            .execute(
                "INSERT INTO tokens VALUES(?1,?2,?3)",
                params![info.id, Sha256::digest(value.as_bytes()).as_slice(), scope],
            )
            .map_err(|_| McpError::Store)?;
        Ok(CreatedToken { info, value })
    }
    /// List metadata only; no reveal or export method exists.
    /// # Errors
    /// Local storage failure.
    pub fn list(&self) -> Result<Vec<TokenInfo>, McpError> {
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let mut query = database
            .prepare("SELECT id,scope FROM tokens ORDER BY id")
            .map_err(|_| McpError::Store)?;
        query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| McpError::Store)?
            .map(|row| {
                let (id, scope) = row.map_err(|_| McpError::Store)?;
                Ok(TokenInfo {
                    id,
                    scope: serde_json::from_str(&scope).map_err(|_| McpError::Store)?,
                })
            })
            .collect()
    }
    /// Permanently revoke and cancel all current work for this credential.
    /// # Errors
    /// Storage failure; live work is cancelled even if persistence fails.
    pub fn revoke(&self, id: &str) -> Result<(), McpError> {
        // Serialize lookup/revoke/client registration so revocation cannot miss an admission.
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        self.disconnect(id);
        database
            .execute("DELETE FROM tokens WHERE id=?1", [id])
            .map_err(|_| McpError::Store)?;
        Ok(())
    }
    /// Disconnect a client; its token remains valid for a later explicit reconnect.
    pub fn disconnect(&self, id: &str) {
        if let Some(client) = self
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id)
        {
            client.cancel.cancel();
        }
    }
    /// Pause all clients on server stop, lock or connection close.
    pub fn cancel_all(&self) {
        for (_, client) in self
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
        {
            client.cancel.cancel();
        }
    }
    /// Bounded connected-client list; no client-supplied names enter it.
    #[must_use]
    pub fn clients(&self) -> Vec<ClientInfo> {
        self.clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(id, client)| ClientInfo {
                id: id.clone(),
                active: client.active,
            })
            .collect()
    }
    pub(crate) fn authenticate(&self, value: &str) -> Result<Grant, McpError> {
        if value.len() != 64 {
            return Err(McpError::Auth);
        }
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let entry: Option<(String, String)> = database
            .query_row(
                "SELECT id,scope FROM tokens WHERE hash=?1",
                [Sha256::digest(value.as_bytes()).as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| McpError::Store)?;
        let (id, scope) = entry.ok_or(McpError::Auth)?;
        let scope: TokenScope = serde_json::from_str(&scope).map_err(|_| McpError::Store)?;
        if scope.expires <= now()? {
            return Err(McpError::Auth);
        }
        let mut clients = self.clients.lock().unwrap_or_else(PoisonError::into_inner);
        let client = clients.entry(id.clone()).or_insert_with(|| Client {
            cancel: Arc::new(CancellationToken::new()),
            active: 0,
        });
        if client.active >= 4 {
            return Err(McpError::Limit);
        }
        client.active += 1;
        Ok(Grant {
            info: TokenInfo { id, scope },
            cancel: client.cancel.clone(),
            store: self.clone(),
        })
    }
}
pub(crate) fn now() -> Result<u32, McpError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| McpError::Invalid)?
        .as_secs()
        .try_into()
        .map_err(|_| McpError::Invalid)
}
fn validate(scope: &TokenScope) -> Result<(), McpError> {
    openrows_ai::validate_id(&scope.provider).map_err(|_| McpError::Invalid)?;
    let now = now()?;
    if scope.connections.is_empty()
        || scope.connections.len() > 32
        || scope.expires <= now
        || scope.expires > now + 30 * 86400
    {
        return Err(McpError::Invalid);
    }
    for id in &scope.connections {
        openrows_ai::validate_id(id).map_err(|_| McpError::Invalid)?;
    }
    Ok(())
}
fn random_id() -> String {
    use std::fmt::Write;
    rand::random::<[u8; 32]>()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;
