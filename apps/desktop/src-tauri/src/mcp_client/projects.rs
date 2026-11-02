// SPDX-License-Identifier: Apache-2.0
//! Commands for Project-scoped MCP servers (docs/23 §3.2). Secret values are
//! typed into the native secure dialog and go straight to the keychain.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
use super::{McpClientTool, State, admit, credentials, dtos, failure, secret_work};
use crate::{
    connections::IpcError,
    security::secret_prompt::{NativePrompt, PromptText, SecretEntry, enter},
};
use openrows_mcp::client::{ClientError, ProjectServer};
use openrows_secrets::{SecretKey, SecretStore as _, SecretString};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

/// Device trust of one declared server.
#[derive(Clone, Copy, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectTrustState {
    /// Never trusted here; it never starts.
    Untrusted,
    /// Its definition changed since it was trusted; trust and values were removed.
    Changed,
    /// The exact definition shown was trusted.
    Trusted,
}

/// A declared server with everything the user reviews before trusting it.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectServerView {
    root: String,
    project: String,
    /// Device key used by the other commands and tool discovery.
    key: String,
    /// Exact command, arguments, URL and the names of required values.
    server: ProjectServer,
    /// Digest of `server`; trusting sends it back so a changed file is refused.
    digest: String,
    trust: ProjectTrustState,
    enabled_tools: Vec<String>,
    tools: Vec<McpClientTool>,
}

/// Register the servers of a Project that was just loaded; nothing starts.
pub(crate) fn register(app: &AppHandle, root: &str, project: &str, servers: &[ProjectServer]) {
    let stale = app
        .state::<State>()
        .register_project(root, project, servers);
    for key in stale {
        tauri::async_runtime::spawn(async move {
            // A changed definition never inherits values typed for the old one.
            let _ = secret_work(move |store| store.delete(&SecretKey::mcp_server(&key))).await;
        });
    }
}

/// Servers of every Project loaded this session, for review in Settings.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_project_servers(app: AppHandle) -> Result<Vec<ProjectServerView>, IpcError> {
    admit(&app)?;
    let state = app.state::<State>();
    let inner = state.lock();
    inner
        .projects
        .iter()
        .map(|entry| {
            let tools = inner
                .tools
                .get(&entry.key)
                .map_or(Ok(Vec::new()), |tools| dtos(tools))?;
            Ok(ProjectServerView {
                root: entry.root.clone(),
                project: entry.project.clone(),
                key: entry.key.clone(),
                digest: entry.server.digest(),
                server: entry.server.clone(),
                trust: match (&entry.trust, entry.changed) {
                    (Some(_), _) => ProjectTrustState::Trusted,
                    (None, true) => ProjectTrustState::Changed,
                    (None, false) => ProjectTrustState::Untrusted,
                },
                enabled_tools: entry
                    .trust
                    .as_ref()
                    .map(|trust| trust.enabled_tools.clone())
                    .unwrap_or_default(),
                tools,
            })
        })
        .collect()
}

/// Trust the reviewed definition; a definition that changed since is refused.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_project_trust(
    app: AppHandle,
    key: String,
    digest: String,
) -> Result<(), IpcError> {
    admit(&app)?;
    app.state::<State>().trust_project(&key, &digest)
}

/// Withdraw trust and delete the server's keychain values.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_project_revoke(app: AppHandle, key: String) -> Result<(), IpcError> {
    admit(&app)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    app.state::<State>().revoke_project(&key)?;
    secret_work(move |store| store.delete(&SecretKey::mcp_server(&key))).await
}

/// Enable exactly these tools of a trusted Project server.
#[tauri::command]
#[specta::specta]
pub(crate) fn mcp_project_tools(
    app: AppHandle,
    key: String,
    enabled: Vec<String>,
) -> Result<(), IpcError> {
    admit(&app)?;
    app.state::<State>().enable_project_tools(&key, enabled)?;
    app.state::<crate::assistant::AssistantState>().cancel(None);
    Ok(())
}

/// Enter one declared variable or header in the native secure dialog.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mcp_project_secret(
    app: AppHandle,
    key: String,
    name: String,
) -> Result<SecretEntry, IpcError> {
    admit(&app)?;
    let entry = app.state::<State>().loaded(&key)?;
    if entry.trust.is_none() || !entry.server.declares(&name) {
        return Err(failure(ClientError::Invalid));
    }
    let text = PromptText {
        title: format!("{name} for {}", entry.server.name),
        message: format!(
            "Value for the MCP server \u{201c}{}\u{201d} of the project \u{201c}{}\u{201d}. \
             OpenRows stores it only in the macOS keychain; the project file never holds it.",
            entry.server.name, entry.project
        ),
    };
    let server = entry.server.clone();
    let id = key.clone();
    let saved = enter(&NativePrompt { app: app.clone() }, &text, move |value| {
        Box::pin(store_secret(id, server, name, value))
    })
    .await?;
    if saved == SecretEntry::Saved {
        app.state::<crate::assistant::AssistantState>().cancel(None);
        app.state::<State>().forget_project_catalogue(&key)?;
    }
    Ok(saved)
}

async fn store_secret(
    key: String,
    server: ProjectServer,
    name: String,
    value: SecretString,
) -> Result<(), IpcError> {
    let current = credentials(&key).await.map_err(IpcError::from)?;
    let next =
        super::project_state::merge_secret(&server, current, &name, value.expose().to_owned())?;
    let json =
        SecretString::new(serde_json::to_string(&next).map_err(|_| failure(ClientError::Invalid))?);
    secret_work(move |store| store.set(&SecretKey::mcp_server(&key), &json)).await
}
