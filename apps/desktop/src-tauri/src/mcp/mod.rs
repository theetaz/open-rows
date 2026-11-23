// SPDX-License-Identifier: Apache-2.0
//! App lifecycle and guarded credential commands for the built-in MCP host.
pub(crate) mod apps;
pub(crate) mod apps_connect;
pub(crate) mod backend;
mod catalog;
mod monitoring;
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
use tauri_specta::Event as _;

/// An external agent called a tool; carries no SQL, rows or error text.
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpActivity {
    event: openrows_mcp::ActivityEvent,
}
impl tauri_specta::Event for McpActivity {
    const NAME: &'static str = "mcp-activity";
}

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
    forward_activity(app.handle(), &host.tokens);
    app.manage(McpState {
        host,
        running: tokio::sync::Mutex::new(None),
    });
    app.manage(proposals::Proposals::default());
    Ok(())
}
/// Push every recorded tool call to the webview so activity appears live.
fn forward_activity(app: &AppHandle, tokens: &TokenStore) {
    let mut events = tokens.activity().subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                // Emitting only fails while the webview is gone.
                Ok(event) => drop(McpActivity { event }.emit(&app)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
/// Saved transport settings.
pub(crate) fn saved(app: &AppHandle) -> Result<McpSettings, IpcError> {
    parse(&settings::current(&app.state::<SettingsFile>()).unwrap_or_default())
}
/// Reconcile saved settings, then the port the server listens on, if any.
pub(crate) async fn running_port(app: &AppHandle) -> Option<u16> {
    // A refusal (off, locked, bind failure) simply leaves the server stopped.
    let _ = synchronize(app).await;
    let state = app.state::<McpState>();
    let running = state.running.lock().await;
    running.as_ref().map(|(_, server)| server.port())
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
    if let Some(state) = app.try_state::<crate::assistant::ScheduledState>() {
        state.interrupt_all();
    }
    if let Some(state) = app.try_state::<crate::assistant::AssistantState>() {
        state.cancel(None);
    }
    if let Some(state) = app.try_state::<McpState>() {
        state.host.tokens.cancel_all();
    }
}
pub(crate) fn exit(app: &AppHandle) {
    pause(app);
    // Ends Codex status probes and sign-ins; their CLI process groups are killed.
    if let Some(state) = app.try_state::<AiState>() {
        state.cancel_all();
    }
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
