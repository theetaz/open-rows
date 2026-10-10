// SPDX-License-Identifier: Apache-2.0
//! Native profile export and core-owned, preview-first public-file imports.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
mod dto;
mod pending;
use super::{
    Connections,
    dto::ProfileDto,
    error::{IpcError, IpcErrorKind},
    profiles::profile_id,
};
use dto::{ImportDecisionDto, ImportPreview, ImportRow, ImportSource};
use openrows_store::profile::{
    ConnectionProfile,
    exchange::{self, ExchangeError, ImportBatch},
    tool_import::{self, ToolFormat},
};
use pending::Pending;
pub(crate) use pending::PendingImports;
use std::{io::Read as _, path::Path};
use tauri::{Manager as _, State};
use tauri_plugin_dialog::DialogExt as _;
impl From<ExchangeError> for IpcError {
    fn from(error: ExchangeError) -> Self {
        match error {
            ExchangeError::Profile(error) => error.into(),
            error => Self::new(IpcErrorKind::Invalid, error.to_string()),
        }
    }
}
/// Export selected ids, or all when empty, to a native-selected JSON file.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn connections_export(
    app: tauri::AppHandle,
    state: State<'_, Connections>,
    ids: Vec<String>,
) -> Result<Option<String>, IpcError> {
    let profiles = selected(&state, &ids)?;
    let text = exchange::export(&profiles)?;
    let path = app
        .dialog()
        .file()
        .set_title("Export connections (no secrets)")
        .add_filter("OpenRows connections", &["json"])
        .set_file_name("openrows-connections.json")
        .blocking_save_file();
    let Some(path) = path else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|_| IpcError::invalid("Choose a local file"))?;
    std::fs::write(&path, text)
        .map_err(|_| IpcError::new(IpcErrorKind::Storage, "Cannot write connection export"))?;
    Ok(Some(path.display().to_string()))
}
fn selected(state: &Connections, ids: &[String]) -> Result<Vec<ConnectionProfile>, IpcError> {
    if ids.is_empty() {
        return Ok(state.profiles.list()?.profiles);
    }
    ids.iter()
        .map(|id| Ok(state.profiles.get(profile_id(id)?)?))
        .collect()
}
/// Parse and validate without writes; passwords remain only in the core preview.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn connections_import_preview(
    app: tauri::AppHandle,
    state: State<'_, Connections>,
    source: ImportSource,
) -> Result<Option<ImportPreview>, IpcError> {
    let Some(batch) = load(&app, source)? else {
        return Ok(None);
    };
    let mut batch = batch;
    crate::security::fallback_import(&mut batch);
    let existing = state.profiles.list()?.profiles;
    let conflicts = exchange::preview(&batch, &existing)?;
    let rows = batch
        .entries
        .iter()
        .zip(conflicts)
        .map(|(entry, conflict)| ImportRow {
            profile: ProfileDto::from(&entry.profile),
            conflict: conflict.into(),
            has_password: entry.password.is_some(),
        })
        .collect();
    let warnings = batch.warnings.clone();
    let token = state.imports.insert(Pending {
        batch,
        snapshot: snapshot(&existing)?,
    })?;
    Ok(Some(ImportPreview {
        token,
        rows,
        warnings,
    }))
}
fn load(app: &tauri::AppHandle, source: ImportSource) -> Result<Option<ImportBatch>, IpcError> {
    let format = match source {
        ImportSource::Pgpass => ToolFormat::Pgpass,
        ImportSource::PgService => ToolFormat::PgService,
        ImportSource::MyCnf => ToolFormat::MyCnf,
        ImportSource::OpenFile => {
            let picked = app
                .dialog()
                .file()
                .set_title("Import OpenRows connections")
                .add_filter("OpenRows connections", &["json"])
                .blocking_pick_file();
            return picked
                .map(|file| {
                    file.into_path()
                        .map_err(|_| IpcError::invalid("Choose a local file"))
                        .and_then(|path| read_open(&path))
                })
                .transpose();
        }
    };
    let home = app
        .path()
        .home_dir()
        .map_err(|_| IpcError::new(IpcErrorKind::Storage, "Cannot resolve home directory"))?;
    let mut batch = ImportBatch::default();
    for path in tool_import::locations(format, &home) {
        let loaded = tool_import::read(format, &path)?;
        batch.entries.extend(loaded.entries);
        batch.warnings.extend(loaded.warnings);
    }
    Ok(Some(batch))
}
fn read_open(path: &Path) -> Result<ImportBatch, IpcError> {
    let file = std::fs::File::open(path)
        .map_err(|_| IpcError::new(IpcErrorKind::Storage, "Cannot read connection import"))?;
    let mut text = String::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|_| IpcError::invalid("Import must be UTF-8 JSON"))?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(IpcError::invalid("Connection import exceeds 4 MiB"));
    }
    Ok(exchange::parse(&text)?)
}
fn snapshot(profiles: &[ConnectionProfile]) -> Result<String, IpcError> {
    serde_json::to_string(profiles)
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "Cannot capture connection preview"))
}
/// Apply explicit decisions to the exact preview; stale previews write nothing.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn connections_import_apply(
    state: State<'_, Connections>,
    token: String,
    decisions: Vec<ImportDecisionDto>,
) -> Result<Vec<ProfileDto>, IpcError> {
    let pending = state.imports.take(&token)?;
    if pending.snapshot != snapshot(&state.profiles.list()?.profiles)? {
        return Err(IpcError::invalid(
            "Connections changed since preview; preview again before importing",
        ));
    }
    let decisions: Vec<_> = decisions.iter().map(Into::into).collect();
    let saved = exchange::apply_import(&state.profiles, pending.batch, &decisions)?;
    Ok(saved.iter().map(ProfileDto::from).collect())
}
/// Drop an unconfirmed preview and its optional passwords.
#[tauri::command]
#[specta::specta]
pub(crate) fn connections_import_cancel(
    state: State<'_, Connections>,
    token: String,
) -> Result<(), IpcError> {
    state.imports.discard(&token)
}
