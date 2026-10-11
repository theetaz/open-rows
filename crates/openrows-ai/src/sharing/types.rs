// SPDX-License-Identifier: Apache-2.0
//! Sharing controls and fixed refusal reasons, safe to expose over IPC.
use serde::{Deserialize, Serialize};

/// Maximum category authorised for a connection; every connection defaults off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SharingLevel {
    /// No database context.
    #[default]
    None,
    /// Names, types and literal-free DDL only.
    SchemaOnly,
    /// Schema plus aggregate statistics and normalised plans.
    SchemaAndStats,
    /// Also bounded, redacted row samples.
    SampleRows,
}

/// Additional schema.table.column rule; built-in rules cannot be disabled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(
    tag = "kind",
    content = "pattern",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum RedactionRule {
    /// Case-insensitive glob with * and ? wildcards.
    Glob(String),
    /// Case-insensitive regular expression.
    Regex(String),
}

/// Device-local policy; production consent never travels with connection exports.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct SharingPolicy {
    /// Maximum authorised sharing.
    pub level: SharingLevel,
    /// Saved provider IDs allowed to receive this context.
    pub providers: Vec<String>,
    /// Require a loopback local-model protocol (reachability is not attestation).
    pub local_only: bool,
    /// Separate permission for monitoring aggregates and plans.
    pub telemetry: bool,
    /// Explicit confirmation recorded by the policy save boundary.
    pub production_confirmed: bool,
    /// Additional identifier and column redactions.
    pub rules: Vec<RedactionRule>,
}

/// Hard bounds, also applied to exact preview JSON including its envelope.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum sampled rows across all tables.
    pub rows: usize,
    /// Maximum sampled cells across all tables.
    pub cells: usize,
    /// UTF-8 bytes per retained field or cell.
    pub field_bytes: usize,
    /// Total serialized UTF-8 bytes.
    pub bytes: usize,
    /// Conservative estimate: at most one token per UTF-8 byte.
    pub tokens: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            rows: 20,
            cells: 200,
            field_bytes: 512,
            bytes: 65536,
            tokens: 16384,
        }
    }
}

/// Payload categories; never user-provided audit labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Explicit user question and fixed database error code.
    Assistant,
    /// Object definitions.
    Schema,
    /// Literal-free statement text.
    Query,
    /// Normalised plan.
    Plan,
    /// Numeric aggregates.
    Stats,
    /// Redacted bounded rows.
    Samples,
}

/// Exact provider context, display-only: it cannot construct a gated payload.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContextPreview {
    /// Exact JSON sent as the provider's input text.
    pub payload: String,
    /// Serialized bytes including JSON escaping/envelope.
    pub bytes: u32,
    /// Conservative token estimate.
    pub tokens: u32,
    /// Actual retained categories.
    pub categories: Vec<Category>,
    /// Whether any size bound removed context.
    pub truncated: bool,
}

/// One append-only outbound attempt; no payload, model text, endpoint or key.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuditEntry {
    /// Unix milliseconds, supplied by the transport.
    pub time: String,
    /// Validated saved provider ID, never its label.
    pub provider: String,
    /// Validated connection ID, never its name.
    pub connection: String,
    /// Authorised sharing level.
    pub level: SharingLevel,
    /// Exact context bytes.
    pub bytes: u32,
    /// SHA-256 of exact context bytes.
    pub hash: String,
    /// Actual retained categories.
    pub categories: Vec<Category>,
}

/// Fixed refusal without interpolated database/provider data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// Global permission off.
    #[error("AI features are off")]
    AiOff,
    /// Requested category exceeds the policy.
    #[error("Data-sharing level is too low")]
    LevelTooLow,
    /// Production requires explicit opt-in.
    #[error("Production sharing requires confirmation")]
    Production,
    /// Provider absent from allowlist.
    #[error("Provider is not allowed for this connection")]
    ProviderNotAllowed,
    /// Local-only with remote endpoint or protocol.
    #[error("This connection requires a local model")]
    LocalOnly,
    /// Independent monitoring permission absent.
    #[error("Telemetry sharing is off")]
    Telemetry,
    /// Bad rule, identifier, input size or limits.
    #[error("Invalid sharing configuration or context")]
    Invalid,
    /// Persistent store failed; outbound transport fails closed.
    #[error("AI sharing store is unavailable")]
    Store,
}
