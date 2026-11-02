// SPDX-License-Identifier: Apache-2.0
//! Window commands: open a new window, list windows, and move tabs between
//! them. Moved tabs travel as the webview's own serialised descriptors (the
//! same allowlisted shape session restore stores); results never move.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects owned command arguments"
)]
use serde::Serialize;
use specta::Type;
use tauri::{Emitter, Manager, State, WebviewWindow};

use super::create::{self, Placement};
use super::{Windows, open, persist, slots};
use crate::connections::{IpcError, IpcErrorKind};

/// Largest moved-tab payload, as for stored workspace state.
const MAX_TRANSFER_BYTES: usize = 32 * 1024 * 1024;

/// One open workspace window, for "Move to Window".
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowEntryDto {
    label: String,
    /// The native title the window's workspace set (its connection).
    title: String,
    /// The window that asked.
    current: bool,
}

fn failed(error: &tauri::Error) -> IpcError {
    IpcError::new(IpcErrorKind::Internal, error.to_string())
}

/// Open a new, empty workspace window offset from `from`; returns its label.
fn open_new(app: &tauri::AppHandle, from: &WebviewWindow) -> Result<String, IpcError> {
    let label = slots::free(&open(app)).ok_or_else(|| {
        IpcError::invalid(format!(
            "OpenRows opens at most {} windows",
            slots::MAX_WINDOWS
        ))
    })?;
    // A reused slot starts empty: its old workspace belonged to a closed window.
    slots::forget(&app.state::<openrows_store::LocalStore>(), &label)
        .map_err(|error| IpcError::new(IpcErrorKind::Storage, error.to_string()))?;
    create::open(app, &label, &Placement::Cascade(from)).map_err(|error| failed(&error))?;
    persist(app);
    Ok(label)
}

/// Window ▸ New Window: an independent workspace window.
#[tauri::command]
#[specta::specta]
pub(crate) async fn window_new(window: WebviewWindow) -> Result<String, IpcError> {
    let app = window.app_handle().clone();
    open_new(&app, &window)
}

/// Every open workspace window, in slot order.
#[tauri::command]
#[specta::specta]
pub(crate) fn window_list(window: WebviewWindow) -> Vec<WindowEntryDto> {
    let app = window.app_handle();
    open(app)
        .into_iter()
        .filter_map(|label| {
            let other = app.get_webview_window(&label)?;
            Some(WindowEntryDto {
                current: label == window.label(),
                title: other.title().unwrap_or_default(),
                label,
            })
        })
        .collect()
}

/// Move tabs to window `target`, or to a new window when `None`. The target
/// takes them with [`window_transfer_take`] and is focused; returns its label.
#[tauri::command]
#[specta::specta]
pub(crate) async fn window_transfer(
    window: WebviewWindow,
    target: Option<String>,
    payload: String,
) -> Result<String, IpcError> {
    if payload.len() > MAX_TRANSFER_BYTES {
        return Err(IpcError::invalid("The moved tabs are larger than 32 MiB"));
    }
    let app = window.app_handle().clone();
    let label = match target {
        Some(label) if label != window.label() && open(&app).contains(&label) => label,
        Some(_) => return Err(IpcError::invalid("That window is no longer open")),
        None => open_new(&app, &window)?,
    };
    app.state::<Windows>()
        .lock()
        .inbox
        .entry(label.clone())
        .or_default()
        .push(payload);
    let _ = app.emit_to(label.as_str(), "window-transfer-ready", ());
    if let Some(target) = app.get_webview_window(&label) {
        target.set_focus().map_err(|error| failed(&error))?;
    }
    Ok(label)
}

/// The tabs moved to the calling window since it last asked.
#[tauri::command]
#[specta::specta]
pub(crate) fn window_transfer_take(
    window: WebviewWindow,
    windows: State<'_, Windows>,
) -> Vec<String> {
    windows
        .lock()
        .inbox
        .remove(window.label())
        .unwrap_or_default()
}
