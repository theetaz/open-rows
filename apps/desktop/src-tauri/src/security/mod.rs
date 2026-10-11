// SPDX-License-Identifier: Apache-2.0
//! Application lock and native execution authentication (FR-APP-11).
mod auth;
mod lock;
use crate::connections::{IpcError, IpcErrorKind};
use auth::{Authenticator, NativeAuthenticator};
use openrows_core::safety::{Gate, SafetyPlan};
use openrows_store::profile::ConnectionProfile;
use serde::Serialize;
use specta::Type;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;
use tauri::{Emitter, Manager, State};

/// App-wide security state, independent of window and session lifetimes.
pub(crate) struct Security {
    lock: Mutex<lock::Lock>,
    started: Instant,
}
impl Default for Security {
    fn default() -> Self {
        Self {
            lock: Mutex::new(lock::Lock::default()),
            started: Instant::now(),
        }
    }
}
impl Security {
    /// Refuse an IPC request while locked, checking the monotonic deadline first.
    pub(crate) fn admit(&self, command: &str) -> Result<(), IpcError> {
        let mut lock = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        lock.tick(self.started.elapsed());
        lock.admit(command)
    }
    /// Apply settings without letting edits unlock the app.
    pub(crate) fn configure(&self, value: &serde_json::Value, startup: bool) {
        let security = if NativeAuthenticator.available() {
            value["security"].clone()
        } else {
            serde_json::Value::Null
        };
        self.lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .configure(&security, startup, self.started.elapsed());
    }
}

/// Native availability and the lock state for all windows.
#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SecurityStatus {
    locked: bool,
    available: bool,
    notice: Option<String>,
}

/// Current lock state; this command never counts as activity.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn security_status(state: State<'_, Security>) -> SecurityStatus {
    let _ = state.admit("security_status");
    let available = NativeAuthenticator.available();
    SecurityStatus {
        locked: state.lock.lock().unwrap_or_else(PoisonError::into_inner).locked,
        available,
        notice: (!available).then(|| "OS authentication is unavailable. Authenticate policies use matching Confirm policies; app locks cannot be enabled.".into()),
    }
}

/// Lock every window immediately.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn security_lock(
    app: tauri::AppHandle,
    state: State<'_, Security>,
) -> Result<(), IpcError> {
    if !NativeAuthenticator.available() {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "OS authentication is unavailable",
        ));
    }
    state
        .lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .lock();
    app.state::<crate::ai::AiState>().revoke();
    let _ = app.emit("security-changed", true);
    Ok(())
}

/// Unlock only after a successful OS authentication for this lock generation.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn security_unlock(
    app: tauri::AppHandle,
    state: State<'_, Security>,
) -> Result<(), IpcError> {
    let generation = state
        .lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .generation();
    auth::authenticate("unlock OpenRows".into()).await?;
    state
        .lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .unlock(generation, state.started.elapsed())?;
    let _ = app.emit("security-changed", false);
    Ok(())
}

/// Report input from a focused window; polling and background work do not reset inactivity.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn security_activity(window: tauri::WebviewWindow, state: State<'_, Security>) {
    if window.is_focused().unwrap_or(false) {
        state
            .lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .input(state.started.elapsed());
    }
}

/// Shared execution boundary for reviewed mutations and ordinary queries.
pub(crate) async fn authorize(
    profile: &ConnectionProfile,
    statements: Vec<String>,
    context: &str,
) -> Result<(), IpcError> {
    let plan = SafetyPlan::check(crate::queries::commands::policy(profile), statements);
    if plan.gate() == Gate::Block {
        return Err(IpcError::invalid(
            "This connection is read-only: nothing was run",
        ));
    }
    authenticate_plan(&plan, &format!("{} ({context})", profile.name)).await?;
    Ok(())
}

/// Drive inactivity locking while the application is in the background.
pub(crate) fn monitor(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut announced = false;
        loop {
            interval.tick().await;
            let state = app.state::<Security>();
            let mut lock = state.lock.lock().unwrap_or_else(PoisonError::into_inner);
            lock.tick(state.started.elapsed());
            if lock.locked != announced {
                announced = lock.locked;
                announce_lock(&app, lock.locked);
            }
        }
    });
}

/// Authenticate this immutable checked plan, never accepting an authentication token from the webview.
pub(crate) async fn authenticate_plan(plan: &SafetyPlan, context: &str) -> Result<(), IpcError> {
    if plan.gate() == Gate::Block {
        return Err(IpcError::invalid(
            "This connection is read-only: nothing was run",
        ));
    }
    if !plan.authentication_required {
        return Ok(());
    }
    auth::authenticate_checked(plan.clone(), format!("execute on {context}")).await
}

/// Native confirmation for batch operations whose existing UI has no token exchange.
pub(crate) async fn confirm_batch(
    app: &tauri::AppHandle,
    profile: &ConnectionProfile,
    statements: Vec<String>,
    context: &str,
) -> Result<(), IpcError> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    let plan = SafetyPlan::check(crate::queries::commands::policy(profile), statements);
    if plan.gate() == Gate::Block {
        return Err(IpcError::invalid(
            "This connection is read-only: nothing was run",
        ));
    }
    if plan.gate() == Gate::Confirm {
        let text = plan
            .statements
            .iter()
            .map(|s| format!("{:?}: {}", s.class.kind, s.sql))
            .collect::<Vec<_>>()
            .join("\n");
        let (sender, receiver) = tokio::sync::oneshot::channel();
        app.dialog()
            .message(format!(
                "Connection: {}\nDatabase: {context}\n\n{text}",
                profile.name
            ))
            .title("Confirm execution")
            .buttons(MessageDialogButtons::OkCancel)
            .show(move |answer| {
                let _ = sender.send(answer);
            });
        if !receiver.await.unwrap_or(false) {
            return Err(IpcError::new(
                IpcErrorKind::Cancelled,
                "Execution cancelled; nothing was run",
            ));
        }
    }
    authenticate_plan(&plan, context).await?;
    app.state::<Security>().admit("query_execute")
}

/// Unsupported platforms import authentication policies as their matching confirmation policies.
pub(crate) fn fallback_import(batch: &mut openrows_store::profile::exchange::ImportBatch) {
    if NativeAuthenticator.available() {
        return;
    }
    for entry in &mut batch.entries {
        if entry.profile.safety.authenticates() {
            entry.profile.safety = entry.profile.safety.fallback();
            batch.warnings.push(
                "OS authentication unavailable: imported policy uses matching Confirm policy"
                    .into(),
            );
        }
    }
}

fn announce_lock(app: &tauri::AppHandle, locked: bool) {
    if locked {
        app.state::<crate::ai::AiState>().revoke();
    }
    let _ = app.emit("security-changed", locked);
}
