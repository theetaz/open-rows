// SPDX-License-Identifier: Apache-2.0
//! Non-secret settings, token scopes and the app-owned tool boundary.
use crate::McpError;
use openrows_ai::{ProviderConfig, sharing::ContextBundle};
use openrows_driver_api::BoxFuture;
use openrows_sql::Dialect;
use serde::{Deserialize, Serialize};

/// Device transport settings; no credential value can be serialized here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct McpSettings {
    /// Explicit permission to start the local bridge.
    pub enabled: bool,
    /// Expose direct Streamable HTTP in addition to the stdio bridge.
    pub http_enabled: bool,
    /// Loopback port, zero asks the OS for an unused port.
    pub port: u16,
}

/// Human-selected read-only credential scope, stored separately from settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenScope {
    /// Saved provider identity for M45-02 allowlist checks.
    pub provider: String,
    /// Explicit saved connection set (at most 32).
    pub connections: Vec<String>,
    /// Absolute Unix expiry in seconds (maximum 30 days).
    pub expires: u32,
}
/// Credential metadata; never includes its value or hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenInfo {
    /// Random non-secret identifier used to revoke the token.
    pub id: String,
    /// Read-only scope and expiry.
    pub scope: TokenScope,
}
/// One-time response from credential creation; no subsequent reveal API exists.
#[derive(Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CreatedToken {
    /// Non-secret metadata.
    pub info: TokenInfo,
    /// Random bearer value, shown once and never persisted.
    pub value: String,
}
/// Connected logical client, identified by its credential rather than untrusted labels.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// Credential ID; one client per credential.
    pub id: String,
    /// Active requests for this client.
    pub active: u32,
}
/// Running transport state; absent port means the bridge is stopped.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    /// OS-selected or configured loopback port.
    pub port: Option<u16>,
    /// Direct HTTP permission, off by default.
    pub http_enabled: bool,
    /// Non-secret read-only token metadata.
    pub tokens: Vec<TokenInfo>,
    /// Connected credential identities.
    pub clients: Vec<ClientInfo>,
}
/// Typed input after tool-specific validation; unknown properties are refused.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInput {
    /// Scoped saved connection ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    /// Database already open in the app.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Schema identifier for object metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Object identifier for details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// Read or proposal SQL (maximum 16 KiB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
}
/// Saved connection/provider identity resolved by the app, never asserted by a client.
pub struct ToolContext {
    /// Current production environment tag.
    pub production: bool,
    /// SQL dialect of the saved driver.
    pub dialect: Dialect,
    /// Current saved provider configuration.
    pub provider: ProviderConfig,
}
/// Native implementation; there is deliberately no general execute or write method.
pub trait ToolBackend: Send + Sync {
    /// Resolve saved identities and refuse locked/off/closed/authenticated work.
    /// # Errors
    /// Refuses invalid identities, lock/off/closed state or required review.
    fn context(&self, provider: &str, input: &ToolInput) -> Result<ToolContext, McpError>;
    /// Read bounded catalog/query/plan context through existing driver paths.
    fn read<'a>(
        &'a self,
        tool: &'a str,
        input: &'a ToolInput,
    ) -> BoxFuture<'a, Result<ContextBundle, McpError>>;
    /// Open model output in an ordinary editor; never submit SQL for execution.
    /// # Errors
    /// Refuses invalid inputs, closed connections or a full proposal queue.
    fn propose(&self, input: &ToolInput) -> Result<(), McpError>;
}
