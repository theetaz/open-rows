// SPDX-License-Identifier: Apache-2.0
//! Apps page: find the user's agent CLIs, read back their `openrows` entry,
//! plan the exact command, test the bridge and list activity. Registration
//! changes live in `apps_connect`. Every probe runs only on a user action.
use super::{McpState, running_port};
use crate::connections::{IpcError, IpcErrorKind};
use openrows_mcp::{
    ActivityEvent, McpError, TokenScope, TokenUsage,
    apps::{self, AgentApp, AppError, AppInstall, BridgeTarget, Registration},
};
use serde::Serialize;
use specta::Type;
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager};

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        let kind = match error {
            AppError::NotInstalled | AppError::BridgeMissing => IpcErrorKind::NotFound,
            AppError::Translocated | AppError::Unrecognised => IpcErrorKind::Unsupported,
            AppError::Timeout => IpcErrorKind::Timeout,
            AppError::Keychain => IpcErrorKind::Secret,
            AppError::Command(_) | AppError::Bridge(_) => IpcErrorKind::Protocol,
        };
        Self::new(kind, error.to_string())
    }
}

/// One agent app: its install and, when connected from here, its credential.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppCard {
    app: AgentApp,
    install: Option<AppInstall>,
    link: Option<LinkView>,
}
/// The registered client id, its credential's scope and usage.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LinkView {
    client: String,
    token: String,
    /// `None` when the credential was revoked elsewhere.
    scope: Option<TokenScope>,
    usage: Option<TokenUsage>,
}
/// Everything the Apps page shows before the slower per-app checks.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppsOverview {
    apps: Vec<AppCard>,
    /// The `openrows-mcp` path entries launch, when it is installed.
    bridge: Option<String>,
    /// Why there is no usable bridge.
    bridge_problem: Option<String>,
    /// Running server port; `None` while it is off.
    port: Option<u16>,
}
/// The tool's own view of the entry and the launch line the server expects.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppCheck {
    registration: Registration,
    expected: Option<String>,
}
/// The commands Connect will run, shown before the user confirms.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppPlan {
    client: String,
    port: u16,
    commands: Vec<String>,
    /// An `openrows` entry already exists and will be replaced.
    replaces: bool,
}

pub(crate) fn bridge() -> Result<PathBuf, AppError> {
    let executable = std::env::current_exe().map_err(|_| AppError::BridgeMissing)?;
    apps::bridge_path(&executable)
}
pub(crate) fn installed(app: AgentApp) -> Result<PathBuf, AppError> {
    apps::locate(app).ok_or(AppError::NotInstalled)
}
/// Run the tool's read-only check and interpret it.
pub(crate) async fn registration(app: AgentApp) -> Result<Registration, AppError> {
    let program = installed(app)?;
    let output = apps::run(&program, &apps::check_args(app), Duration::from_secs(30)).await?;
    apps::parse_check(app, &output)
}

/// Detect the three CLIs (path and `--version`) and read local links.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_apps(app: AppHandle) -> Result<AppsOverview, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_apps")?;
    let port = running_port(&app).await;
    let (claude, codex, gemini) = tokio::join!(
        apps::detect(AgentApp::Claude),
        apps::detect(AgentApp::Codex),
        apps::detect(AgentApp::Gemini)
    );
    let installs = [claude, codex, gemini];
    let tokens = &app.state::<McpState>().host.tokens;
    let (links, scopes, usage) = (tokens.links()?, tokens.list()?, tokens.usage()?);
    let cards = AgentApp::ALL
        .into_iter()
        .zip(installs)
        .map(|(agent, install)| AppCard {
            app: agent,
            install,
            link: links
                .iter()
                .find(|link| link.app == agent.id())
                .map(|link| LinkView {
                    client: link.client.clone(),
                    token: link.token.clone(),
                    scope: scopes
                        .iter()
                        .find(|token| token.id == link.token)
                        .map(|token| token.scope.clone()),
                    usage: usage.iter().find(|item| item.token == link.token).cloned(),
                }),
        })
        .collect();
    let bridge = bridge();
    Ok(AppsOverview {
        apps: cards,
        bridge: bridge
            .as_ref()
            .ok()
            .map(|path| path.to_string_lossy().into_owned()),
        bridge_problem: bridge.err().map(|error| error.to_string()),
        port,
    })
}

/// Ask the tool for its `openrows` entry (Claude and Gemini health-check it).
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_check(app: AppHandle, agent: AgentApp) -> Result<AppCheck, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_check")?;
    let registration = registration(agent).await?;
    let expected = target(&app, agent).await.ok().map(|target| target.line());
    Ok(AppCheck {
        registration,
        expected,
    })
}

/// The launch line of the linked client against the running server.
async fn target(app: &AppHandle, agent: AgentApp) -> Result<BridgeTarget, IpcError> {
    let link = super::apps_connect::link_of(app, agent)?.ok_or(McpError::Invalid)?;
    let port = running_port(app).await.ok_or(McpError::Cancelled)?;
    Ok(BridgeTarget {
        bridge: bridge()?,
        port,
        client: link.client,
    })
}

/// Pick a client id and stable port and list the exact commands to run.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_plan(app: AppHandle, agent: AgentApp) -> Result<AppPlan, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_plan")?;
    let program = installed(agent)?;
    let target = BridgeTarget {
        bridge: bridge()?,
        port: apps::stable_port(super::saved(&app)?.port, running_port(&app).await),
        client: apps::new_client_id(agent),
    };
    let replaces = registration(agent).await?.present;
    let name = program.to_string_lossy().into_owned();
    let mut commands = Vec::new();
    if replaces {
        commands.push(apps::display(&name, &apps::remove_args(agent)));
    }
    commands.push(apps::display(&name, &apps::add_args(agent, &target)));
    Ok(AppPlan {
        client: target.client,
        port: target.port,
        commands,
        replaces,
    })
}

/// Call `list_connections` through the registered launch line, as the app would.
#[tauri::command]
#[specta::specta]
pub(crate) async fn agent_app_test(app: AppHandle, agent: AgentApp) -> Result<String, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_app_test")?;
    let target = target(&app, agent).await?;
    let text = apps::test_bridge(&target).await?;
    Ok(text.chars().take(2000).collect())
}

/// Recent external tool calls, newest first.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn agent_activity(app: AppHandle) -> Result<Vec<ActivityEvent>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("agent_activity")?;
    Ok(app.state::<McpState>().host.tokens.activity().recent(None))
}
