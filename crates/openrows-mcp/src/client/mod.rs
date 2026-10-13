// SPDX-License-Identifier: Apache-2.0
//! User-server client. No database handles, app roots, sampling or write callbacks.
mod config;
mod redact;
mod transport;
pub use config::{Credentials, Endpoint, ServerConfig};
use openrows_ai::Cancellation;
pub use redact::redact_json;
use rmcp::model::{CallToolRequestParams, CallToolResult, PaginatedRequestParams};
use serde::Serialize;
use std::{collections::HashSet, time::Duration};

/// Safe, fixed failures; protocol/command/header error text is never exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// Invalid public metadata or secret-map shape.
    #[error("Invalid MCP client configuration")]
    Invalid,
    /// Tool is disabled or its discovered schema changed.
    #[error("MCP tool is disabled or changed")]
    Disabled,
    /// No matching native first-use consent.
    #[error("MCP tool requires confirmation")]
    Confirmation,
    /// Transport failed, with details withheld.
    #[error("MCP server transport failed")]
    Transport,
    /// Five-second exchange deadline expired.
    #[error("MCP request timed out")]
    Timeout,
    /// Caller cancelled before or during the exchange.
    #[error("MCP request cancelled")]
    Cancelled,
    /// Catalogue, payload or nesting bound exceeded.
    #[error("MCP response exceeds limits")]
    Limit,
}
/// Bounded tool catalogue; descriptions are treated as untrusted server data.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTool {
    /// Validated untrusted server tool name.
    pub name: String,
    /// Bounded input schema.
    pub schema: serde_json::Value,
}
/// Session-only consent is bound to the full configuration and tool schema.
#[derive(Clone, Default)]
pub struct Consent(HashSet<String>);
impl Consent {
    /// Forget grants after lock, settings/credential change, or disable.
    pub fn clear(&mut self) {
        self.0.clear();
    }
    /// Validate enablement before prompting or launching any transport.
    /// # Errors
    /// Disabled tools cannot request confirmation.
    pub fn check(&self, config: &ServerConfig, tool: &ClientTool) -> Result<(), ClientError> {
        enabled(config, tool)?;
        if self.0.contains(&identity(config, tool)?) {
            Ok(())
        } else {
            Err(ClientError::Confirmation)
        }
    }
    /// Record only an explicit human response for an enabled tool.
    /// # Errors
    /// Reject disabled or invalid tools/configurations.
    pub fn confirm(&mut self, config: &ServerConfig, tool: &ClientTool) -> Result<(), ClientError> {
        enabled(config, tool)?;
        self.0.insert(identity(config, tool)?);
        Ok(())
    }
}
fn enabled(config: &ServerConfig, tool: &ClientTool) -> Result<(), ClientError> {
    config.validate()?;
    if !config.enabled_tools.contains(&tool.name) {
        return Err(ClientError::Disabled);
    }
    Ok(())
}
fn identity(config: &ServerConfig, tool: &ClientTool) -> Result<String, ClientError> {
    serde_json::to_string(&(config, tool)).map_err(|_| ClientError::Invalid)
}
/// Discover at most 64 tools across a bounded eight-page catalogue.
/// # Errors
/// Fixed timeout, cancellation, transport or size failure.
pub async fn discover(
    config: &ServerConfig,
    credentials: &Credentials,
    cancel: &Cancellation,
) -> Result<Vec<ClientTool>, ClientError> {
    bounded(cancel, async {
        let session = transport::connect(config, credentials).await?;
        catalogue(&session.service, credentials).await
    })
    .await
}
/// Confirmed invocation parameters, kept outside the transport control plane.
pub struct CallInput<'a> {
    /// Discovered tool.
    pub tool: &'a ClientTool,
    /// Redacted argument object.
    pub arguments: serde_json::Value,
    /// Native session grant.
    pub consent: &'a Consent,
}
/// Call an enabled and confirmed tool. No `OpenRows` database capability is supplied.
/// # Errors
/// Fixed consent, timeout, cancellation, transport or size failure.
pub async fn call(
    config: &ServerConfig,
    credentials: &Credentials,
    input: CallInput<'_>,
    cancel: &Cancellation,
) -> Result<CallToolResult, ClientError> {
    input.consent.check(config, input.tool)?;
    if !input.arguments.is_object()
        || serde_json::to_vec(&input.arguments)
            .map_err(|_| ClientError::Invalid)?
            .len()
            > 16384
    {
        return Err(ClientError::Limit);
    }
    bounded(cancel, async {
        let session = transport::connect(config, credentials).await?;
        let current = catalogue(&session.service, credentials).await?;
        if !current.iter().any(|tool| tool == input.tool) {
            return Err(ClientError::Disabled);
        }
        let result = session
            .service
            .call_tool(
                CallToolRequestParams::new(input.tool.name.clone()).with_arguments(
                    input
                        .arguments
                        .as_object()
                        .cloned()
                        .ok_or(ClientError::Invalid)?,
                ),
            )
            .await
            .map_err(|_| ClientError::Transport)?;
        if serde_json::to_vec(&result)
            .map_err(|_| ClientError::Transport)?
            .len()
            > 65536
        {
            return Err(ClientError::Limit);
        }
        Ok(result)
    })
    .await
}
async fn bounded<T>(
    cancel: &Cancellation,
    work: impl Future<Output = Result<T, ClientError>>,
) -> Result<T, ClientError> {
    tokio::select! { biased; () = cancel.cancelled() => Err(ClientError::Cancelled), result = tokio::time::timeout(Duration::from_secs(5), work) => result.map_err(|_| ClientError::Timeout)? }
}

async fn catalogue(
    service: &rmcp::Peer<rmcp::RoleClient>,
    credentials: &Credentials,
) -> Result<Vec<ClientTool>, ClientError> {
    let mut tools = Vec::new();
    let mut cursor = None;
    for _ in 0..8 {
        let result = service
            .list_tools(
                cursor.map(|cursor| PaginatedRequestParams::default().with_cursor(Some(cursor))),
            )
            .await
            .map_err(|_| ClientError::Transport)?;
        for tool in result.tools {
            let schema = serde_json::Value::Object((*tool.input_schema).clone());
            if !config::tool_name(&tool.name)
                || redact::contains_secret(&tool.name, credentials)
                || serde_json::to_vec(&schema)
                    .map_err(|_| ClientError::Invalid)?
                    .len()
                    > 8192
                || tools.len() >= 64
                || tools
                    .iter()
                    .any(|saved: &ClientTool| saved.name == tool.name)
            {
                return Err(ClientError::Limit);
            }
            tools.push(ClientTool {
                name: tool.name.to_string(),
                schema: redact::public_schema(&schema, credentials, false, 0)?,
            });
        }
        cursor = result.next_cursor;
        if cursor.is_none() {
            return Ok(tools);
        }
    }
    Err(ClientError::Limit)
}
