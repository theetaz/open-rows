// SPDX-License-Identifier: Apache-2.0
//! Transactional local policies and append-only metadata with bounded retention.
use super::{AuditEntry, RedactionRule, Refusal, SharingLevel, SharingPolicy};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::{Arc, Mutex, PoisonError},
};
use tokio::sync::watch;

/// Local `SQLite` policy/audit store; clones share revisions and the serialized writer.
#[derive(Clone)]
pub struct SharingStore {
    pub(super) database: Arc<Mutex<Connection>>,
    pub(super) revision: watch::Sender<u64>,
    pub(super) retention: u32,
}
impl SharingStore {
    /// Open/create private local storage; no provider payload is ever persisted.
    /// # Errors
    /// Fixed store refusal on filesystem, schema or database failure.
    pub fn open(path: &Path) -> Result<Self, Refusal> {
        let connection = Connection::open(path).map_err(|_| Refusal::Store)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Refusal::Store)?;
        }
        Self::new(connection, 1000)
    }
    /// Isolated in-memory storage for tests and embedded consumers.
    /// # Errors
    /// Fixed store refusal if initialization fails or retention is invalid.
    pub fn memory(retention: u32) -> Result<Self, Refusal> {
        Self::new(
            Connection::open_in_memory().map_err(|_| Refusal::Store)?,
            retention,
        )
    }
    fn new(connection: Connection, retention: u32) -> Result<Self, Refusal> {
        if !(1..=1000).contains(&retention) {
            return Err(Refusal::Invalid);
        }
        connection.execute_batch("CREATE TABLE IF NOT EXISTS sharing(connection TEXT PRIMARY KEY, policy TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS audit(id INTEGER PRIMARY KEY, entry TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS investigations(id INTEGER PRIMARY KEY, entry TEXT NOT NULL);").map_err(|_| Refusal::Store)?;
        Ok(Self {
            database: Arc::new(Mutex::new(connection)),
            revision: watch::channel(0).0,
            retention,
        })
    }
    /// Read policy; missing IDs always return the default None policy.
    /// # Errors
    /// Refuses invalid IDs or damaged/unavailable storage.
    pub fn policy(&self, connection: &str) -> Result<SharingPolicy, Refusal> {
        use rusqlite::OptionalExtension;
        valid_id(connection)?;
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let text: Option<String> = database
            .query_row(
                "SELECT policy FROM sharing WHERE connection=?1",
                [connection],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| Refusal::Store)?;
        text.map(|text| serde_json::from_str(&text).map_err(|_| Refusal::Store))
            .transpose()
            .map(Option::unwrap_or_default)
    }
    /// Save a policy only after explicit production acknowledgement, revoking old payloads.
    /// # Errors
    /// Refuses invalid rules/providers or unconfirmed production sharing.
    pub fn save(
        &self,
        connection: &str,
        mut policy: SharingPolicy,
        production: bool,
    ) -> Result<(), Refusal> {
        valid_id(connection)?;
        validate_policy(&policy)?;
        if production && policy.level != SharingLevel::None && !policy.production_confirmed {
            return Err(Refusal::Production);
        }
        // A non-production save cannot pre-authorise a future production retag.
        policy.production_confirmed &= production && policy.level != SharingLevel::None;
        let text = serde_json::to_string(&policy).map_err(|_| Refusal::Invalid)?;
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        database.execute("INSERT INTO sharing VALUES(?1, ?2) ON CONFLICT(connection) DO UPDATE SET policy=excluded.policy", params![connection, text]).map_err(|_| Refusal::Store)?;
        self.revision.send_modify(|revision| *revision += 1);
        Ok(())
    }
    pub(super) fn append(&self, entry: &AuditEntry) -> Result<(), Refusal> {
        let text = serde_json::to_string(entry).map_err(|_| Refusal::Store)?;
        let mut database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let transaction = database.transaction().map_err(|_| Refusal::Store)?;
        transaction
            .execute("INSERT INTO audit(entry) VALUES(?1)", [text])
            .map_err(|_| Refusal::Store)?;
        transaction.execute("DELETE FROM audit WHERE id NOT IN (SELECT id FROM audit ORDER BY id DESC LIMIT ?1)", [self.retention]).map_err(|_| Refusal::Store)?;
        transaction.commit().map_err(|_| Refusal::Store)
    }
    /// Newest-first bounded audit entries.
    /// # Errors
    /// Fixed refusal on damaged or unavailable storage.
    pub fn audit(&self) -> Result<Vec<AuditEntry>, Refusal> {
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let mut statement = database
            .prepare("SELECT entry FROM audit ORDER BY id DESC LIMIT ?1")
            .map_err(|_| Refusal::Store)?;
        let rows = statement
            .query_map([self.retention], |row| row.get::<_, String>(0))
            .map_err(|_| Refusal::Store)?;
        rows.map(|row| {
            serde_json::from_str(&row.map_err(|_| Refusal::Store)?).map_err(|_| Refusal::Store)
        })
        .collect()
    }
    /// Explicitly purge audit history without changing connection policies.
    /// # Errors
    /// Fixed refusal on unavailable storage.
    pub fn purge(&self) -> Result<(), Refusal> {
        self.database
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .execute_batch("DELETE FROM audit; DELETE FROM investigations;")
            .map_err(|_| Refusal::Store)?;
        Ok(())
    }
}
pub(super) fn valid_id(id: &str) -> Result<(), Refusal> {
    crate::validate_id(id).map_err(|_| Refusal::Invalid)
}
fn validate_policy(policy: &SharingPolicy) -> Result<(), Refusal> {
    if policy.providers.len() > 32 || policy.rules.len() > 32 {
        return Err(Refusal::Invalid);
    }
    for id in &policy.providers {
        valid_id(id)?;
    }
    for rule in &policy.rules {
        let pattern = match rule {
            RedactionRule::Glob(pattern) | RedactionRule::Regex(pattern) => pattern,
        };
        if pattern.is_empty() || pattern.len() > 512 {
            return Err(Refusal::Invalid);
        }
    }
    super::redact::Redactor::new(
        &policy.rules,
        super::Limits::default(),
        openrows_sql::Dialect::Postgres,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AiControl, AiError, HttpProvider, NetworkOptions, Provider, ProviderConfig, ProviderKind,
        RequestOptions, TextRequest,
    };
    #[tokio::test]
    async fn broken_audit_store_refuses_outbound_before_network()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = SharingStore::memory(1000)?;
        store.save(
            "connection",
            SharingPolicy {
                level: SharingLevel::SchemaOnly,
                providers: vec!["fixture".into()],
                ..SharingPolicy::default()
            },
            false,
        )?;
        let config = ProviderConfig {
            id: "fixture".into(),
            name: "Fixture".into(),
            kind: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:1/".into(),
            allow_insecure_remote: false,
            secret_ref: String::new(),
        };
        let control = AiControl::default();
        control.set_enabled(true);
        let context = store.gate(super::super::GateRequest {
            connection: "connection",
            production: false,
            provider: &config,
            control: &control,
            dialect: openrows_sql::Dialect::Postgres,
            bundle: super::super::ContextBundle::default(),
        })?;
        store
            .database
            .lock()
            .unwrap()
            .execute("DROP TABLE audit", [])?;
        let provider = HttpProvider::new(&config, None, control, NetworkOptions::default())?;
        let (sink, _receiver) = tokio::sync::mpsc::channel(1);
        let request = TextRequest {
            model: "model".into(),
            context,
            max_output: 10,
        };
        assert_eq!(
            provider
                .stream(&request, sink, &RequestOptions::default())
                .await,
            Err(AiError::Audit)
        );
        Ok(())
    }
}
