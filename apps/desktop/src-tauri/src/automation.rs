// SPDX-License-Identifier: Apache-2.0
//! Private, opt-in IPC transport for native E2E builds. Never compiled in release.
mod observations;
mod transport;
pub(crate) use observations::{query as record_query, window as record_window};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Listener, Manager};
use tokio::sync::oneshot;

pub(crate) struct Automation {
    pub(crate) root: PathBuf,
    pending: Mutex<Option<oneshot::Sender<serde_json::Value>>>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AutomationError {
    #[error("Automation root must be an existing absolute directory with an ownership marker")]
    InvalidRoot,
    #[error("Automation I/O: {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(root) = std::env::var_os("OPENROWS_E2E_ROOT") else {
        return Ok(());
    };
    let root = validate_root(&PathBuf::from(root))?;
    observations::install();
    let socket = root.join("bridge.sock");
    // std binding works before Tauri starts its asynchronous runtime.
    let listener = std::os::unix::net::UnixListener::bind(socket)?;
    listener.set_nonblocking(true)?;
    app.manage(Automation {
        root,
        pending: Mutex::new(None),
    });
    app.add_capability(r#"{"identifier":"automation","windows":["main"],"permissions":["allow-automation-reply","allow-automation-observations"]}"#)?;
    let handle = app.handle().clone();
    let window = app
        .get_webview_window("main")
        .ok_or("Missing main window")?;
    window.once("openrows:first-paint-ready", move |_| {
        tauri::async_runtime::spawn(transport::serve(handle, listener));
    });
    Ok(())
}

fn validate_root(root: &Path) -> Result<PathBuf, AutomationError> {
    if !root.is_absolute() || !root.is_dir() || !root.join(".automation-owned").is_file() {
        return Err(AutomationError::InvalidRoot);
    }
    Ok(root.canonicalize()?)
}

pub(crate) fn root(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.try_state::<Automation>()
        .map(|state| state.root.clone())
}

pub(crate) fn handler(invoke: tauri::ipc::Invoke<tauri::Wry>) -> bool {
    let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![automation_reply, observations::automation_observations];
    handler(invoke)
}

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ReplyError {
    #[error("Automation state is unavailable")]
    State,
    #[error("No automation request is pending")]
    NoRequest,
    #[error("Automation request expired")]
    Expired,
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects owned command arguments"
)]
#[tauri::command]
fn automation_reply(
    state: tauri::State<'_, Automation>,
    value: serde_json::Value,
) -> Result<(), ReplyError> {
    let sender = state
        .pending
        .lock()
        .map_err(|_| ReplyError::State)?
        .take()
        .ok_or(ReplyError::NoRequest)?;
    sender.send(value).map_err(|_| ReplyError::Expired)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automation_requires_an_explicit_owned_root() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR"))?;
        assert!(validate_root(dir.path()).is_err());
        assert!(validate_root(Path::new("relative")).is_err());
        std::fs::write(dir.path().join(".automation-owned"), "")?;
        assert_eq!(validate_root(dir.path())?, dir.path().canonicalize()?);
        Ok(())
    }
}
