// SPDX-License-Identifier: Apache-2.0
//! Scrubbed run accounting; raw questions, model text, SQL and payloads have no field.
use super::{Refusal, SharingStore};
use serde::{Deserialize, Serialize};
use std::sync::PoisonError;

/// Bounded metadata record for an investigation attempt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvestigationAudit {
    /// Epoch milliseconds.
    pub time: String,
    /// Saved connection identity.
    pub connection: String,
    /// Saved provider identity.
    pub provider: String,
    /// Explicit selected model identifier.
    pub model: String,
    /// Static tools used; never arguments.
    pub tools: Vec<String>,
    /// Delivered opaque references only.
    pub evidence: Vec<String>,
    /// Conservative accounted token upper bound, not provider-reported billing.
    pub tokens: u32,
    /// Estimated conservative cost in millionths.
    pub cost_micros: u32,
    /// running, completed, cancelled, refused or failed.
    pub outcome: String,
}
impl SharingStore {
    /// Persist metadata before/after a run, retaining newest entries only.
    /// # Errors
    /// Invalid metadata or storage failure; sending must fail closed.
    pub fn record_investigation(&self, entry: &InvestigationAudit) -> Result<(), Refusal> {
        super::store::valid_id(&entry.connection)?;
        super::store::valid_id(&entry.provider)?;
        if entry.model.is_empty()
            || entry.model.len() > 256
            || entry.time.parse::<u64>().is_err()
            || entry.tokens > 65536
            || entry.tools.len() > 12
            || entry
                .tools
                .iter()
                .any(|tool| !crate::scheduled::SCHEDULE_TOOLS.contains(&tool.as_str()))
            || entry.evidence.len() > 200
            || entry
                .evidence
                .iter()
                .any(|id| id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
            || !["running", "completed", "cancelled", "refused", "failed"]
                .contains(&entry.outcome.as_str())
        {
            return Err(Refusal::Invalid);
        }
        let text = serde_json::to_string(entry).map_err(|_| Refusal::Invalid)?;
        let mut database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let tx = database.transaction().map_err(|_| Refusal::Store)?;
        tx.execute("INSERT INTO investigations(entry) VALUES(?1)", [text])
            .map_err(|_| Refusal::Store)?;
        tx.execute("DELETE FROM investigations WHERE id NOT IN (SELECT id FROM investigations ORDER BY id DESC LIMIT ?1)", [self.retention]).map_err(|_| Refusal::Store)?;
        tx.commit().map_err(|_| Refusal::Store)
    }
    /// Bounded newest-first metadata, including incomplete attempts after a crash.
    /// # Errors
    /// Store failure.
    pub fn investigations(&self) -> Result<Vec<InvestigationAudit>, Refusal> {
        let database = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let mut query = database
            .prepare("SELECT entry FROM investigations ORDER BY id DESC LIMIT ?1")
            .map_err(|_| Refusal::Store)?;
        query
            .query_map([self.retention], |row| row.get::<_, String>(0))
            .map_err(|_| Refusal::Store)?
            .map(|row| {
                serde_json::from_str(&row.map_err(|_| Refusal::Store)?).map_err(|_| Refusal::Store)
            })
            .collect()
    }
}
