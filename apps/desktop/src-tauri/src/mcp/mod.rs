// SPDX-License-Identifier: Apache-2.0
//! App lifecycle and guarded credential commands for the built-in MCP host.
mod backend;
mod catalog;
pub(crate) mod proposals;
use crate::{
    ai::AiState,
    connections::{Connections, IpcError, IpcErrorKind},
    settings::{self, SettingsFile},
};
use openrows_mcp::{
    CreatedToken, McpError, McpHost, McpSettings, RunningServer, ServerStatus, TokenScope,
    TokenStore,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub(crate) struct McpState {
    host: McpHost,
    running: tokio::sync::Mutex<Option<(McpSettings, RunningServer)>>,
}
impl From<McpError> for IpcError {
    fn from(error: McpError) -> Self {
        let kind = match error {
            McpError::Auth => IpcErrorKind::Auth,
            McpError::Cancelled => IpcErrorKind::Cancelled,
            McpError::Invalid => IpcErrorKind::Invalid,
            McpError::Store => IpcErrorKind::Storage,
            McpError::Transport => IpcErrorKind::Network,
            McpError::Database => IpcErrorKind::Protocol,
            McpError::Limit => IpcErrorKind::Timeout,
            McpError::Sharing | McpError::Unsafe | McpError::Review => IpcErrorKind::Unsupported,
        };
        Self::new(kind, error.to_string())
    }
}
pub(crate) fn install(app: &mut tauri::App) -> Result<(), McpError> {
    let data = &app.state::<crate::locations::Locations>().data;
    let host = McpHost {
        tokens: TokenStore::open(&data.join("mcp-tokens.sqlite"))?,
        sharing: app
            .state::<openrows_ai::sharing::SharingStore>()
            .inner()
            .clone(),
        control: app.state::<AiState>().control.clone(),
        backend: Arc::new(backend::Backend(app.handle().clone())),
    };
    app.manage(McpState {
        host,
        running: tokio::sync::Mutex::new(None),
    });
    app.manage(proposals::Proposals::default());
    Ok(())
}
pub(crate) fn parse(value: &serde_json::Value) -> Result<McpSettings, IpcError> {
    value
        .get("mcp")
        .map(|value| serde_json::from_value(value.clone()).map_err(|_| McpError::Invalid.into()))
        .transpose()
        .map(Option::unwrap_or_default)
}
pub(crate) fn configure(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = synchronize(&app).await;
    });
}
pub(crate) fn pause(app: &AppHandle) {
    if let Some(state) = app.try_state::<McpState>() {
        state.host.tokens.cancel_all();
    }
}
pub(crate) fn exit(app: &AppHandle) {
    pause(app);
    if let Some(state) = app.try_state::<McpState>()
        && let Ok(mut running) = state.running.try_lock()
    {
        running.take();
    }
}
async fn synchronize(app: &AppHandle) -> Result<(), IpcError> {
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    let settings = parse(&value)?;
    let state = app.state::<McpState>();
    let mut running = state.running.lock().await;
    let allowed = settings.enabled
        && state.host.control.enabled()
        && app
            .state::<crate::security::Security>()
            .admit("mcp_read")
            .is_ok();
    if !allowed {
        running.take();
        return Ok(());
    }
    if running
        .as_ref()
        .is_some_and(|(known, _)| known == &settings)
    {
        return Ok(());
    }
    running.take();
    let server = state.host.start(settings.clone()).await?;
    *running = Some((settings, server));
    Ok(())
}
/// Reconcile saved transport settings and return only scrubbed status/metadata.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_status(app: AppHandle) -> Result<ServerStatus, IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_status")?;
    synchronize(&app).await?;
    let state = app.state::<McpState>();
    let running = state.running.lock().await;
    Ok(ServerStatus {
        port: running.as_ref().map(|(_, server)| server.port()),
        http_enabled: running
            .as_ref()
            .is_some_and(|(settings, _)| settings.http_enabled),
        tokens: state.host.tokens.list()?,
        clients: state.host.tokens.clients(),
    })
}
/// Return a new read-only credential once after validating all saved identities.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn mcp_token_create(
    app: AppHandle,
    scope: TokenScope,
) -> Result<CreatedToken, IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_token_create")?;
    crate::ai::config(&app, &scope.provider)?;
    for id in &scope.connections {
        app.state::<Connections>()
            .profiles
            .get(crate::connections::profiles::profile_id(id)?)?;
    }
    Ok(app.state::<McpState>().host.tokens.create(scope)?)
}
/// Revoke a credential and cancel its current requests.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn mcp_token_revoke(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_token_revoke")?;
    Ok(app.state::<McpState>().host.tokens.revoke(&id)?)
}
/// Disconnect a credential's client immediately; a later reconnect needs its valid token.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_client_disconnect(app: AppHandle, id: String) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_client_disconnect")?;
    app.state::<McpState>().host.tokens.disconnect(&id);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_defaults_off_and_settings_refuse_credentials() {
        let settings = parse(&serde_json::json!({})).unwrap();
        assert!(!settings.enabled && !settings.http_enabled);
        assert!(parse(&serde_json::json!({"mcp":{"token":"test-only"}})).is_err());
    }
}
