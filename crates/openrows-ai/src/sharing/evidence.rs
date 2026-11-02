// SPDX-License-Identifier: Apache-2.0
//! Monitoring disclosure has numeric values and fixed states, never arbitrary source text.
use serde::{Deserialize, Serialize};

/// An opaque, timestamped sample reference. Producers hash native sample identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[cfg_attr(feature = "ipc-types", specta(inline))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    /// SHA-256 of connection, generation, sample identity and time.
    pub id: String,
    /// Application metric identifier, never SQL or a server error.
    pub metric: String,
    /// Collection epoch milliseconds.
    pub collected_ms: f64,
    /// Finite observed value; absent for every gap.
    pub value: Option<f64>,
    /// Explicit validity/visibility.
    pub state: EvidenceState,
    /// Database/server or opaque query/session/plan scope.
    pub scope: String,
}
/// No raw error, activity user, address, SQL or wait text is representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[cfg_attr(feature = "ipc-types", specta(inline))]
#[serde(rename_all = "camelCase")]
pub enum EvidenceState {
    /// Observed numeric sample.
    Ok,
    /// Missing baseline or NULL.
    Unknown,
    /// Collector interval missing.
    Gap,
    /// Source absent.
    Unavailable,
    /// Source visibility denied.
    PermissionDenied,
    /// Counters reset.
    CounterReset,
    /// Partial source coverage.
    Partial,
    /// Per-connection redaction removed the value.
    Redacted,
}
impl Evidence {
    pub(super) fn validate(&self) -> Result<(), super::Refusal> {
        if self.id.len() != 64
            || !self.id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.metric.is_empty()
            || self.metric.len() > 128
            || !self
                .metric
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_:.-".contains(&byte))
            || !scope(&self.scope)
            || !self.collected_ms.is_finite()
            || self.collected_ms < 0.0
            || self.value.is_some_and(|value| !value.is_finite())
            || (self.state != EvidenceState::Ok && self.value.is_some())
        {
            return Err(super::Refusal::Invalid);
        }
        Ok(())
    }
}

fn scope(value: &str) -> bool {
    if ["database", "server", "unknown"].contains(&value) {
        return true;
    }
    if let Some((kind, id)) = value.split_once(':') {
        return ["query", "session", "plan"].contains(&kind)
            && id.len() == 64
            && id.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    let Some((waiting, blocking)) = value.split_once('>') else {
        return false;
    };
    waiting.len() == 24
        && blocking.len() == 64
        && waiting
            .bytes()
            .chain(blocking.bytes())
            .all(|byte| byte.is_ascii_hexdigit())
}
