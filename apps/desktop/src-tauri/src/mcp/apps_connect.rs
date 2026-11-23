// SPDX-License-Identifier: Apache-2.0
//! Connect, renew and disconnect an agent app. Registration always goes
//! through the tool's own `mcp add`/`mcp remove`; the per-client token is
//! written to the keychain by the bridge, so no secret enters the tool's
//! configuration. Failures roll the credential back.
use super::{
    McpState,
    apps::{bridge, installed, registration},
    running_port,
};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_mcp::{
    AppLink, McpError, TokenScope,
    apps::{self, AgentApp, AppError, BridgeTarget},
};
use serde::Deserialize;
use specta::Type;
use std::{path::Path, time::Duration};
use tauri::{AppHandle, Manager};

const COMMAND_DEADLINE: Duration = Duration::from_secs(60);

/// A confirmed plan plus the credential scope chosen in the sheet.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ConnectRequest {
    app: AgentApp,
    client: String,
    port: u16,
    provider: String,
    connections: Vec<String>,
}

pub(crate) fn link_of(app: &AppHandle, agent: AgentApp) -> Result<Option<AppLink>, IpcError> {
    Ok(app
        .state::<McpState>()
        .host
        .tokens
        .links()?
        .into_iter()
        .find(|link| link.app == agent.id()))
}
fn expiry() -> Result<u32, IpcError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| McpError::Invalid)?
        .as_secs();
    let now = u32::try_from(now).map_err(|_| McpError::Invalid)?;
    Ok(now + apps::APP_TOKEN_SECONDS)
}
fn validate(app: &AppHandle, request: &ConnectRequest) -> Result<(), IpcError> {
    if !apps::client_matches(request.app, &request.client) || request.port == 0 {
        return Err(McpError::Invalid.into());
    }
    crate::ai::config(app, &request.provider)?;
    for id in &request.connections {
        app.state::<Connections>()
            .profiles
            .get(crate::connections::profiles::profile_id(id)?)?;
    }
    Ok(())
}
/// The last non-empty line of a failed command, as the tool printed it.
fn failure(text: &str) -> AppError {
    let line = text
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("no message");
    AppError::Command(line.chars().take(200).collect())
}
async fn run_tool(program: &Path, args: &[String]) -> Result<(), AppError> {
    let output = apps::run(program, args, COMMAND_DEADLINE).await?;
    if output.success {
        Ok(())
    } else {
        Err(failure(&output.text))
    }
}

/// Issue a token, let the bridge store it, register with the tool, then link.
/// The saved MCP settings must already be on at `port` (the click is the consent).
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_connect(
    app: AppHandle,
    request: ConnectRequest,
) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_connect")?;
    validate(&app, &request)?;
    let program = installed(request.app)?;
    if running_port(&app).await != Some(request.port) {
        return Err(IpcError::new(
            IpcErrorKind::Network,
            "The MCP server is not running on the planned port; turn it on and try again.",
        ));
    }
    let target = BridgeTarget {
        bridge: bridge()?,
        port: request.port,
        client: request.client.clone(),
    };
    release(&app, request.app, false).await;
    let tokens = app.state::<McpState>().host.tokens.clone();
    let created = tokens.create(TokenScope {
        provider: request.provider.clone(),
        connections: request.connections.clone(),
        expires: expiry()?,
    })?;
    let token = created.info.id.clone();
    if let Err(error) = register(&program, request.app, &target, &created.value).await {
        let _ = apps::forget_token(&target.bridge, &target.client).await;
        let _ = tokens.revoke(&token);
        return Err(error.into());
    }
    tokens.link(&AppLink {
        client: request.client,
        app: request.app.id().into(),
        token,
    })?;
    Ok(())
}
async fn register(
    program: &Path,
    agent: AgentApp,
    target: &BridgeTarget,
    value: &str,
) -> Result<(), AppError> {
    apps::store_token(&target.bridge, &target.client, value).await?;
    if registration(agent).await?.present {
        run_tool(program, &apps::remove_args(agent)).await?;
    }
    run_tool(program, &apps::add_args(agent, target)).await
}

/// Revoke the linked credential, delete its keychain item and forget the link;
/// with `unregister`, also run the tool's `mcp remove`. Returns a note when the
/// tool's entry could not be removed.
async fn release(app: &AppHandle, agent: AgentApp, unregister: bool) -> Option<String> {
    let link = link_of(app, agent).ok().flatten()?;
    let mut note = None;
    if unregister {
        let removed = match installed(agent) {
            Ok(program) => run_tool(&program, &apps::remove_args(agent)).await,
            Err(error) => Err(error),
        };
        note = removed.err().map(|error| error.to_string());
    }
    let tokens = &app.state::<McpState>().host.tokens;
    let _ = tokens.revoke(&link.token);
    if let Ok(path) = bridge() {
        let _ = apps::forget_token(&path, &link.client).await;
    }
    let _ = tokens.unlink(&link.client);
    note
}

/// Remove the tool's entry and revoke this app's access.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_disconnect(
    app: AppHandle,
    agent: AgentApp,
) -> Result<Option<String>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_disconnect")?;
    Ok(release(&app, agent, true).await)
}

/// Replace an expiring credential with the same scope; the tool's entry is unchanged.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_renew(app: AppHandle, agent: AgentApp) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_renew")?;
    let link = link_of(&app, agent)?.ok_or(McpError::Invalid)?;
    let tokens = app.state::<McpState>().host.tokens.clone();
    let scope = tokens
        .list()?
        .into_iter()
        .find(|token| token.id == link.token)
        .map(|token| token.scope)
        .ok_or(McpError::Auth)?;
    let created = tokens.create(TokenScope {
        expires: expiry()?,
        ..scope
    })?;
    if let Err(error) = apps::store_token(&bridge()?, &link.client, &created.value).await {
        let _ = tokens.revoke(&created.info.id);
        return Err(error.into());
    }
    tokens.link(&AppLink {
        token: created.info.id,
        ..link.clone()
    })?;
    tokens.revoke(&link.token)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_keep_only_the_tools_last_line() {
        assert_eq!(
            failure("Adding…\nMCP server openrows already exists in user config\n\n"),
            AppError::Command("MCP server openrows already exists in user config".into())
        );
        assert_eq!(failure(""), AppError::Command("no message".into()));
        assert_eq!(
            failure(&"x".repeat(500)),
            AppError::Command("x".repeat(200))
        );
    }
}
