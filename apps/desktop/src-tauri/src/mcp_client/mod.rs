// SPDX-License-Identifier: Apache-2.0
//! Explicit user server setup, keychain-only secrets and native confirmation responses.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
pub(crate) mod router;
mod state;
use crate::{
    ai::AiState,
    connections::{IpcError, IpcErrorKind},
};
use openrows_ai::Cancellation;
use openrows_mcp::client::{ClientError, Credentials, ServerConfig};
use openrows_secrets::{
    CachingSecretStore, KeychainSecretStore, SecretKey, SecretStore, SecretString,
};
use serde::{Deserialize, Serialize};
use specta::Type;
pub(crate) use state::State;
use tauri::{AppHandle, Manager};

pub(crate) fn failure(error: ClientError) -> IpcError {
    let kind = match error {
        ClientError::Timeout => IpcErrorKind::Timeout,
        ClientError::Cancelled => IpcErrorKind::Cancelled,
        ClientError::Transport => IpcErrorKind::Network,
        _ => IpcErrorKind::Invalid,
    };
    IpcError::new(kind, error.to_string())
}
pub(crate) fn config(app: &AppHandle, id: &str) -> Result<ServerConfig, IpcError> {
    app.state::<State>()
        .lock()
        .configs
        .iter()
        .find(|config| config.id == id)
        .cloned()
        .ok_or_else(|| failure(ClientError::Invalid))
}
fn admit(app: &AppHandle) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("mcp_client")?;
    Ok(())
}
/// List non-secret device-local preferences; never start a server on mount.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_servers_list(app: AppHandle) -> Result<Vec<ServerConfig>, IpcError> {
    admit(&app)?;
    Ok(app.state::<State>().lock().configs.clone())
}
/// Save public preferences and invalidate tool catalogues/consent immediately.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_servers_save(app: AppHandle, servers: Vec<ServerConfig>) -> Result<(), IpcError> {
    admit(&app)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    app.state::<State>().save(servers)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    Ok(())
}
/// Secret inputs are write-only; IPC has no reveal command.
#[derive(Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub(crate) struct McpSecretInput {
    env: std::collections::BTreeMap<String, String>,
    headers: std::collections::BTreeMap<String, String>,
}
/// Replace environment/headers in the OS keychain, without settings/logging them.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_server_secrets(
    app: AppHandle,
    id: String,
    input: McpSecretInput,
) -> Result<(), IpcError> {
    admit(&app)?;
    config(&app, &id)?;
    let credentials = Credentials {
        env: input.env,
        headers: input.headers,
    };
    credentials.validate().map_err(failure)?;
    let value = SecretString::new(
        serde_json::to_string(&credentials).map_err(|_| failure(ClientError::Invalid))?,
    );
    app.state::<crate::assistant::AssistantState>().cancel(None);
    app.state::<State>().invalidate_credentials(&id);
    let secret_id = id.clone();
    let result =
        secret_work(move |store| store.set(&SecretKey::mcp_server(&secret_id), &value)).await;
    // Also discard discovery admitted while the OS credential prompt was open.
    app.state::<State>().invalidate_credentials(&id);
    app.state::<crate::assistant::AssistantState>().cancel(None);
    result
}
/// Remove keychain values before removing the public server reference.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_server_delete(app: AppHandle, id: String) -> Result<(), IpcError> {
    admit(&app)?;
    config(&app, &id)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    app.state::<State>().invalidate();
    let secret_id = id.clone();
    secret_work(move |store| store.delete(&SecretKey::mcp_server(&secret_id))).await?;
    let configs = app
        .state::<State>()
        .lock()
        .configs
        .iter()
        .filter(|item| item.id != id)
        .cloned()
        .collect();
    app.state::<State>().save(configs)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    Ok(())
}
/// Public catalogue DTO keeps schema JSON lossless across the JavaScript boundary.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpClientTool {
    name: String,
    schema: String,
}
/// Explicit connection probe, gated by AI enablement and a five-second deadline.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_server_tools(
    app: AppHandle,
    id: String,
) -> Result<Vec<McpClientTool>, IpcError> {
    admit(&app)?;
    let config = config(&app, &id)?;
    let revision = app.state::<State>().lock().revision;
    let control = app.state::<AiState>().control.clone();
    let options = openrows_ai::RequestOptions {
        timeout: std::time::Duration::from_secs(5),
        cancel: Cancellation::default(),
    };
    control
        .run(&options, async {
            let secrets = credentials(&id).await?;
            let tools = openrows_mcp::client::discover(&config, &secrets, &options.cancel)
                .await
                .map_err(|_| openrows_ai::AiError::Network)?;
            Ok(tools)
        })
        .await
        .map_err(IpcError::from)
        .and_then(|tools| {
            admit(&app)?;
            let state = app.state::<State>();
            let mut inner = state.lock();
            if inner.revision != revision || !inner.configs.contains(&config) {
                return Err(failure(ClientError::Cancelled));
            }
            inner.tools.insert(id, tools.clone());
            tools
                .into_iter()
                .map(|tool| {
                    Ok(McpClientTool {
                        name: tool.name,
                        schema: serde_json::to_string(&tool.schema)
                            .map_err(|_| failure(ClientError::Invalid))?,
                    })
                })
                .collect()
        })
}
pub(super) async fn credentials(id: &str) -> Result<Credentials, openrows_ai::AiError> {
    let id = id.to_owned();
    let secret = secret_work(move |store| store.get(&SecretKey::mcp_server(&id)))
        .await
        .map_err(|_| openrows_ai::AiError::Auth)?;
    let value = secret.map_or_else(|| "{}".into(), |value| value.expose().to_owned());
    serde_json::from_str(&value).map_err(|_| openrows_ai::AiError::InvalidConfig)
}
async fn secret_work<T: Send + 'static>(
    work: impl FnOnce(
        CachingSecretStore<KeychainSecretStore>,
    ) -> Result<T, openrows_secrets::SecretError>
    + Send
    + 'static,
) -> Result<T, IpcError> {
    tauri::async_runtime::spawn_blocking(move || {
        work(CachingSecretStore::new(KeychainSecretStore::new()))
    })
    .await
    .map_err(|_| secret_error())?
    .map_err(|_| secret_error())
}
fn secret_error() -> IpcError {
    IpcError::new(IpcErrorKind::Secret, "MCP keychain operation failed")
}
/// Resolve only a currently pending native prompt; unsolicited grants are refused.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_tool_confirm(app: AppHandle, id: String, allow: bool) -> Result<(), IpcError> {
    admit(&app)?;
    let send = app
        .state::<State>()
        .lock()
        .pending
        .remove(&id)
        .ok_or_else(|| failure(ClientError::Confirmation))?;
    send.send(allow)
        .map_err(|_| failure(ClientError::Cancelled))
}
