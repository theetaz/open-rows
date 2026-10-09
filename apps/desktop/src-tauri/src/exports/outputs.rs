// SPDX-License-Identifier: Apache-2.0
//! Open or reveal what a finished export wrote. Only paths this run's
//! exports produced are accepted, so the webview cannot open arbitrary files.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

use tauri::State;

use crate::connections::{IpcError, IpcErrorKind};

/// Files and folders written by finished exports.
#[derive(Default)]
pub(crate) struct ExportOutputs(Mutex<HashSet<PathBuf>>);

impl ExportOutputs {
    pub(crate) fn record(&self, path: PathBuf) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(path);
    }

    fn known(&self, path: &str) -> Result<PathBuf, IpcError> {
        let path = PathBuf::from(path);
        let known = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&path);
        if known {
            Ok(path)
        } else {
            Err(IpcError::new(
                IpcErrorKind::NotFound,
                "Only files written by an export can be opened",
            ))
        }
    }
}

/// The platform command that opens `path`, or shows it selected in the file manager.
fn launcher(path: &Path, reveal: bool) -> Command {
    let mut command;
    if cfg!(target_os = "macos") {
        command = Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg(path);
    } else if cfg!(target_os = "windows") {
        command = Command::new("explorer");
        if reveal {
            let mut select = std::ffi::OsString::from("/select,");
            select.push(path);
            command.arg(select);
        } else {
            command.arg(path);
        }
    } else {
        command = Command::new("xdg-open");
        let shown = if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        command.arg(shown);
    }
    command
}

async fn launch(path: PathBuf, reveal: bool) -> Result<(), IpcError> {
    let status = tauri::async_runtime::spawn_blocking(move || launcher(&path, reveal).status())
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?;
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(IpcError::new(
            IpcErrorKind::Internal,
            format!("The file manager exited with {status}"),
        )),
        Err(error) => Err(IpcError::new(IpcErrorKind::Storage, error.to_string())),
    }
}

/// Show an export's file or folder selected in Finder.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_reveal(
    outputs: State<'_, ExportOutputs>,
    path: String,
) -> Result<(), IpcError> {
    launch(outputs.known(&path)?, true).await
}

/// Open an export's file (or folder) with its default application.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_open(
    outputs: State<'_, ExportOutputs>,
    path: String,
) -> Result<(), IpcError> {
    launch(outputs.known(&path)?, false).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_recorded_outputs_are_accepted() {
        let outputs = ExportOutputs::default();
        assert!(outputs.known("/etc/passwd").is_err());
        outputs.record(PathBuf::from("/tmp/out.csv"));
        assert!(outputs.known("/tmp/out.csv").is_ok());
    }
}
