// SPDX-License-Identifier: Apache-2.0
//! Data locations change only on restart, before `SQLite` or profile stores open.
mod plan;

use crate::connections::{IpcError, IpcErrorKind};
use serde::Serialize;
use specta::Type;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Startup-resolved data directory and fixed platform anchor.
pub(crate) struct Locations {
    pub(crate) data: PathBuf,
    anchor: PathBuf,
    failure: Option<String>,
}

/// Display paths and pending move status; contains no credentials.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationsDto {
    data: String,
    history: String,
    logs: String,
    pending: Option<String>,
    failure: Option<String>,
}

/// Apply a requested offline move, preserving the original on failure.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let anchor = app.path().app_data_dir()?;
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    let anchor = crate::automation::root(app.handle()).map_or(anchor, |root| root.join("data"));
    fs::create_dir_all(&anchor)?;
    let pointer = anchor.join("data-location");
    let source = fs::read_to_string(&pointer).map_or_else(|_| anchor.clone(), PathBuf::from);
    let pending = anchor.join("pending-data-location");
    let mut data = source.clone();
    let mut failure = None;
    if let Ok(path) = fs::read_to_string(&pending) {
        let target = PathBuf::from(path);
        match checkpoint(&source).and_then(|()| plan::copy_then_switch(&source, &target, &pointer))
        {
            Ok(()) => data = target,
            Err(error) => failure = Some(format!("Data move rolled back: {error}")),
        }
        fs::remove_file(pending)?;
    }
    app.manage(Locations {
        data,
        anchor,
        failure,
    });
    Ok(())
}

/// Recover WAL and checkpoint before copying the closed local store.
fn checkpoint(source: &Path) -> Result<(), plan::MoveError> {
    let path = source.join("openrows.sqlite");
    if !path.exists() {
        return Ok(());
    }
    let store = openrows_store::LocalStore::open(&path)
        .map_err(|e| plan::MoveError::Invalid(e.to_string()))?;
    store
        .checkpoint()
        .map_err(|e| plan::MoveError::Invalid(e.to_string()))
}

/// Current data directory and history/log locations.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command arguments are injected by value"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn data_locations(
    app: tauri::AppHandle,
    locations: State<'_, Locations>,
) -> Result<LocationsDto, IpcError> {
    let logs = app
        .path()
        .app_log_dir()
        .map_err(|error| IpcError::new(IpcErrorKind::Storage, error.to_string()))?;
    Ok(LocationsDto {
        data: locations.data.display().to_string(),
        history: locations.data.join("openrows.sqlite").display().to_string(),
        logs: logs.display().to_string(),
        pending: fs::read_to_string(locations.anchor.join("pending-data-location")).ok(),
        failure: locations.failure.clone(),
    })
}

/// Choose an empty data folder; validation happens again when scheduled and on restart.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_location_choose(
    window: tauri::WebviewWindow,
) -> Result<Option<String>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Choose empty local data folder")
        .pick_folder(move |picked| {
            let _ = sender.send(picked);
        });
    let picked = receiver
        .await
        .map_err(|e| IpcError::new(IpcErrorKind::Internal, e.to_string()))?;
    picked
        .map(|file| {
            file.into_path()
                .map(|path| path.display().to_string())
                .map_err(|e| IpcError::invalid(e.to_string()))
        })
        .transpose()
}

/// Schedule an explicitly approved copy, including local history, for the next restart.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_location_apply(
    locations: State<'_, Locations>,
    path: String,
) -> Result<(), IpcError> {
    let source = locations.data.clone();
    let pending = locations.anchor.join("pending-data-location");
    tauri::async_runtime::spawn_blocking(move || {
        let target = plan::validate(&source, Path::new(&path))?;
        plan::write_pointer(&pending, &target)
    })
    .await
    .map_err(|e| IpcError::new(IpcErrorKind::Internal, e.to_string()))?
    .map_err(|e| IpcError::new(IpcErrorKind::Storage, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpointed_copy_retains_recent_wal_history_and_drafts()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        fs::create_dir(&source)?;
        let target = root.path().join("target");
        fs::create_dir(&target)?;
        let store = openrows_store::LocalStore::open(&source.join("openrows.sqlite"))?;
        store.set_meta("recent-draft", "SELECT 1")?;
        assert!(fs::metadata(source.join("openrows.sqlite-wal"))?.len() > 0);
        store.checkpoint()?;
        assert_eq!(fs::metadata(source.join("openrows.sqlite-wal"))?.len(), 0);
        drop(store);
        checkpoint(&source)?;
        plan::copy_then_switch(&source, &target, &root.path().join("pointer"))?;
        let restored = openrows_store::LocalStore::open(&target.join("openrows.sqlite"))?;
        assert_eq!(restored.meta("recent-draft")?.as_deref(), Some("SELECT 1"));
        Ok(())
    }
}
