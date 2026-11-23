// SPDX-License-Identifier: Apache-2.0
//! Bounded, content-free record of external tool calls for the Apps page.
//!
//! Events carry only the fixed tool name, a validated connection id, the
//! credential id and an outcome class: never SQL, rows or error text.
use crate::McpError;
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError},
};
use tokio::sync::broadcast;

/// Recent events kept in memory for the activity lists.
const RECENT: usize = 200;

/// Result class of one external tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ActivityOutcome {
    /// The tool returned data or opened a proposal.
    Ok,
    /// Policy, review, safety or credential checks refused it.
    Refused,
    /// Cancelled, timed out or failed in the database or transport.
    Failed,
}
impl ActivityOutcome {
    pub(crate) fn of(result: &Result<String, McpError>) -> Self {
        match result {
            Ok(_) => Self::Ok,
            Err(McpError::Sharing | McpError::Unsafe | McpError::Review | McpError::Auth) => {
                Self::Refused
            }
            Err(_) => Self::Failed,
        }
    }
}

/// One external tool call, attributed to its credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    /// Unix seconds.
    pub at: u32,
    /// Credential id that made the call.
    pub token: String,
    /// Agent app registered with this credential (`claude`, `codex`, `gemini`).
    pub app: Option<String>,
    /// Fixed tool name, or `unknown`.
    pub tool: String,
    /// Validated connection id from the arguments, when present.
    pub connection: Option<String>,
    /// Result class.
    pub outcome: ActivityOutcome,
}

/// Persisted per-credential counters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    /// Credential id.
    pub token: String,
    /// Tool calls made with this credential.
    pub calls: u32,
    /// Last request of any kind, Unix seconds.
    pub seen: Option<u32>,
    /// Client name and version as reported by the client (untrusted label).
    pub client: Option<String>,
}

/// In-memory ring of recent events plus a live broadcast for the UI.
#[derive(Clone)]
pub struct ActivityLog {
    recent: Arc<Mutex<VecDeque<ActivityEvent>>>,
    sender: broadcast::Sender<ActivityEvent>,
}
impl Default for ActivityLog {
    fn default() -> Self {
        Self {
            recent: Arc::default(),
            sender: broadcast::channel(64).0,
        }
    }
}
impl ActivityLog {
    /// Live events; slow receivers skip ahead rather than block the server.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ActivityEvent> {
        self.sender.subscribe()
    }
    /// Newest first, optionally only one credential's events.
    #[must_use]
    pub fn recent(&self, token: Option<&str>) -> Vec<ActivityEvent> {
        self.recent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .rev()
            .filter(|event| token.is_none_or(|id| event.token == id))
            .cloned()
            .collect()
    }
    pub(crate) fn push(&self, event: ActivityEvent) {
        {
            let mut recent = self.recent.lock().unwrap_or_else(PoisonError::into_inner);
            if recent.len() == RECENT {
                recent.pop_front();
            }
            recent.push_back(event.clone());
        }
        // No receiver simply means no window is listening.
        let _ = self.sender.send(event);
    }
    /// Drop a revoked credential's events.
    pub(crate) fn forget(&self, token: &str) {
        self.recent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|event| event.token != token);
    }
}

/// A short printable client label: `name version`, at most 64 characters.
pub(crate) fn label(name: &str, version: &str) -> Option<String> {
    let text: String = format!("{name} {version}")
        .chars()
        .filter(|character| character.is_ascii_graphic() || *character == ' ')
        .take(64)
        .collect();
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(token: &str) -> ActivityEvent {
        ActivityEvent {
            at: 1,
            token: token.into(),
            app: None,
            tool: "list_schemas".into(),
            connection: None,
            outcome: ActivityOutcome::Ok,
        }
    }

    #[test]
    fn ring_is_bounded_newest_first_and_filterable() {
        let log = ActivityLog::default();
        let mut live = log.subscribe();
        log.push(event("a"));
        assert_eq!(
            live.try_recv().map(|item| item.token).ok(),
            Some("a".into())
        );
        for index in 1..=RECENT {
            log.push(event(if index % 2 == 0 { "a" } else { "b" }));
        }
        assert_eq!(log.recent(None).len(), RECENT);
        assert!(log.recent(Some("a")).iter().all(|item| item.token == "a"));
        log.forget("a");
        assert!(log.recent(Some("a")).is_empty());
    }

    #[test]
    fn outcomes_and_labels_carry_no_payload() {
        assert_eq!(ActivityOutcome::of(&Ok("x".into())), ActivityOutcome::Ok);
        assert_eq!(
            ActivityOutcome::of(&Err(McpError::Sharing)),
            ActivityOutcome::Refused
        );
        assert_eq!(
            ActivityOutcome::of(&Err(McpError::Limit)),
            ActivityOutcome::Failed
        );
        assert_eq!(
            label("claude-code", "2.1\n"),
            Some("claude-code 2.1".into())
        );
        assert_eq!(label("", ""), None);
        assert_eq!(
            label(&"x".repeat(100), "1").map(|text| text.len()),
            Some(64)
        );
    }
}
