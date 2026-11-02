// SPDX-License-Identifier: Apache-2.0
//! Codex CLI status and sign-in through the CLI itself, in a dedicated
//! per-provider directory under app data.
use super::{AiState, config, start};
use crate::connections::IpcError;
use openrows_ai::{
    AiError, Cancellation, Health, ProviderConfig, RequestOptions, Verification,
    codex::{Bridge, CodexStatus},
};
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager, ipc::Channel};

const PROBE: Duration = Duration::from_secs(30);
const SIGN_IN: Duration = Duration::from_secs(300);

fn root(app: &AppHandle, config: &ProviderConfig) -> PathBuf {
    app.state::<crate::locations::Locations>()
        .data
        .join("ai-cli")
        .join("codex")
        .join(&config.id)
}
pub(super) fn bridge(app: &AppHandle, config: &ProviderConfig) -> Result<Bridge, IpcError> {
    Ok(Bridge::new(config, root(app, config))?)
}
/// Removing a provider also removes the CLI state the bridge created for it.
pub(super) fn forget(app: &AppHandle, config: &ProviderConfig) -> Result<(), IpcError> {
    match std::fs::remove_dir_all(root(app, config)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(IpcError::new(
            crate::connections::IpcErrorKind::Storage,
            "Codex data could not be removed",
        )),
        _ => Ok(()),
    }
}
/// Discovery, version and account state; no prompt is sent.
pub(super) async fn verify(
    app: &AppHandle,
    config: &ProviderConfig,
    cancel: Cancellation,
) -> Result<Verification, IpcError> {
    let (status, models) = bridge(app, config)?
        .status(&RequestOptions {
            timeout: PROBE,
            cancel,
        })
        .await;
    let health = match &status {
        CodexStatus::SignedIn { .. } => Health::Verified,
        CodexStatus::SignedOut { .. } => Health::Failed {
            reason: AiError::Auth,
        },
        CodexStatus::NotInstalled | CodexStatus::Unsupported { .. } => Health::Failed {
            reason: AiError::Unsupported,
        },
        CodexStatus::Error { reason } => Health::Failed {
            reason: reason.clone(),
        },
    };
    Ok(Verification {
        health,
        models,
        usage: None,
        cli: Some(status),
    })
}
/// Run the CLI's own `ChatGPT` sign-in. The browser opens natively and the
/// URL is streamed once so it can be copied; tokens stay with the CLI.
#[tauri::command]
#[specta::specta]
pub(crate) async fn codex_login(
    app: AppHandle,
    id: String,
    on_url: Channel<String>,
) -> Result<CodexStatus, IpcError> {
    app.state::<crate::security::Security>()
        .admit("codex_login")?;
    let config = config(&app, &id)?;
    let state = app.state::<AiState>();
    state.configure(
        &crate::settings::current(&app.state::<crate::settings::SettingsFile>())
            .unwrap_or_default(),
    );
    if !state.control.enabled() {
        return Err(AiError::Disabled.into());
    }
    let (_run, cancel) = start(&state, &id)?;
    let bridge = bridge(&app, &config)?;
    let options = RequestOptions {
        timeout: SIGN_IN,
        cancel,
    };
    let (send, receive) = tokio::sync::oneshot::channel::<String>();
    let forward = async {
        if let Ok(url) = receive.await {
            open(url.clone()).await;
            let _ = on_url.send(url);
        }
    };
    let (status, ()) = tokio::join!(
        state.control.run(&options, bridge.login(&options, send)),
        forward
    );
    Ok(status?)
}
/// Open the validated HTTPS sign-in page in the default browser.
async fn open(url: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        let mut command = if cfg!(target_os = "macos") {
            std::process::Command::new("open")
        } else if cfg!(target_os = "windows") {
            std::process::Command::new("explorer")
        } else {
            std::process::Command::new("xdg-open")
        };
        command
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    })
    .await;
}
