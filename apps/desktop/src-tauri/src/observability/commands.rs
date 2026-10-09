// SPDX-License-Identifier: Apache-2.0
//! Diagnostics IPC: the crash status of this launch, the logs folder, and
//! errors and timings reported by the webview.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use specta::Type;
use tauri::State;

use super::Diagnostics;
use super::scrub::clip;
use crate::connections::{IpcError, IpcErrorKind};

/// Longest webview error message kept, in characters.
const MAX_MESSAGE: usize = 500;
/// Longest webview error or command name kept, in characters.
const MAX_NAME: usize = 120;

/// The program that shows a folder in the file manager.
#[cfg(target_os = "macos")]
const REVEALER: &str = "open";
#[cfg(target_os = "windows")]
const REVEALER: &str = "explorer";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const REVEALER: &str = "xdg-open";

/// Whether this launch follows a run that did not exit cleanly.
#[tauri::command]
#[specta::specta]
pub(crate) fn crash_recovery_status(diagnostics: State<'_, Diagnostics>) -> bool {
    diagnostics.crashed
}

/// What the webview reports: an error, a rejected promise, or a timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiReportKind {
    Error,
    UnhandledRejection,
    LongTask,
    SlowIpc,
}

/// One webview report. `name` is the error name or the command; `message`
/// is free text and is scrubbed and clipped before it is written.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiReport {
    kind: UiReportKind,
    name: String,
    message: Option<String>,
    duration_ms: Option<f64>,
}

/// Log one webview report under the `ui` target.
#[tauri::command]
#[specta::specta]
pub(crate) fn ui_report(report: UiReport) {
    let name = clip(&report.name, MAX_NAME);
    let message = clip(report.message.as_deref().unwrap_or_default(), MAX_MESSAGE);
    let duration_ms = report.duration_ms.unwrap_or_default();
    match report.kind {
        UiReportKind::Error | UiReportKind::UnhandledRejection => {
            tracing::warn!(target: "ui", kind = ?report.kind, %name, %message, "webview error");
        }
        UiReportKind::LongTask => {
            tracing::info!(target: "ui", %name, duration_ms, "main thread task");
        }
        UiReportKind::SlowIpc => {
            tracing::info!(target: "ui", %name, duration_ms, "slow ipc command");
        }
    }
}

/// Show the log folder in the file manager (Help ▸ Open Logs Folder).
///
/// # Errors
/// The folder cannot be created, or the file manager cannot be started.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_logs_folder(diagnostics: State<'_, Diagnostics>) -> Result<(), IpcError> {
    let dir = diagnostics.log_dir.clone();
    fs::create_dir_all(&dir).map_err(|error| storage(&dir, &error))?;
    let target = dir.clone();
    let status =
        tauri::async_runtime::spawn_blocking(move || Command::new(REVEALER).arg(&target).status())
            .await
            .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?;
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(IpcError::new(
            IpcErrorKind::Internal,
            format!("{REVEALER} exited with {status}"),
        )),
        Err(error) => Err(storage(&dir, &error)),
    }
}

fn storage(path: &Path, error: &io::Error) -> IpcError {
    IpcError::new(
        IpcErrorKind::Storage,
        format!("{}: {error}", path.display()),
    )
}
