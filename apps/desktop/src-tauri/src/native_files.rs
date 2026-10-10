// SPDX-License-Identifier: Apache-2.0
//! Native document handoff is queued until the webview is ready; never runs SQL.
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::queries::files::OpenedSqlFileDto;
use serde::Serialize;
use specta::Type;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use tauri::{Emitter, Manager, State};

/// Bounded native paths awaiting frontend handoff (not SQL execution requests).
#[derive(Default)]
pub(crate) struct NativeFiles(Mutex<VecDeque<PathBuf>>);

/// Validated content or an actionable rejection, never executable instructions.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum NativeFileDto {
    Sql { opened: OpenedSqlFileDto },
    Database { path: String },
    Rejected { message: String },
}

/// Queue OS-selected paths and announce readiness without exposing file contents.
pub(crate) fn enqueue(app: &tauri::AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    let state = app.state::<NativeFiles>();
    let mut pending = state.0.lock().unwrap_or_else(PoisonError::into_inner);
    for path in paths {
        if pending.len() < 32 && !pending.contains(&path) {
            pending.push_back(path);
        }
    }
    let _ = app.emit("native-files-ready", ());
}

/// Startup argv document paths on platforms without macOS Opened events.
pub(crate) fn install(app: &mut tauri::App) {
    #[cfg(not(target_os = "macos"))]
    enqueue(
        app.handle(),
        std::env::args_os()
            .skip(1)
            .filter(|arg| !arg.to_string_lossy().starts_with('-'))
            .map(PathBuf::from),
    );
    #[cfg(target_os = "macos")]
    let _ = app;
}

/// Drain queued files after startup or a native event; lock guard applies at IPC.
#[tauri::command]
#[specta::specta]
pub(crate) async fn native_files_take(
    files: State<'_, NativeFiles>,
) -> Result<Vec<NativeFileDto>, IpcError> {
    let paths: Vec<_> = files
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .map(|path| match sniff(path) {
                Ok(file) => file,
                Err(error) => NativeFileDto::Rejected {
                    message: error.message,
                },
            })
            .collect()
    })
    .await
    .map_err(|e| IpcError::new(IpcErrorKind::Internal, e.to_string()))
}

fn sniff(path: &Path) -> Result<NativeFileDto, IpcError> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if extension == "duckdb" {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "DuckDB coming in M2-04",
        ));
    }
    if !path.is_absolute() || !path.is_file() {
        return Err(IpcError::invalid("Choose an existing local file"));
    }
    if extension == "sql" {
        return OpenedSqlFileDto::read(path).map(|opened| NativeFileDto::Sql { opened });
    }
    if !["sqlite", "sqlite3", "db"].contains(&extension.as_str()) {
        return Err(IpcError::invalid(
            "Unexpected file type; use SQLite or UTF-8 .sql",
        ));
    }
    let mut header = [0; 16];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|_| IpcError::invalid("This file is not a SQLite database"))?;
    if &header != b"SQLite format 3\0" {
        return Err(IpcError::invalid("This file is not a SQLite database"));
    }
    Ok(NativeFileDto::Database {
        path: path.display().to_string(),
    })
}

/// Reveal only the local database path of an existing saved connection.
#[tauri::command]
#[specta::specta]
pub(crate) async fn database_reveal(
    connections: State<'_, Connections>,
    profile_id: String,
) -> Result<(), IpcError> {
    let profile = connections
        .profiles
        .get(crate::connections::profiles::profile_id(&profile_id)?)?;
    let path = profile
        .file
        .map(|file| file.path)
        .ok_or_else(|| IpcError::invalid("This connection has no local database file"))?;
    crate::exports::outputs::launch(path, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sniff_accepts_sqlite_and_utf8_sql_without_execution_and_rejects_junk()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let db = root.path().join("data.sqlite3");
        std::fs::write(&db, b"SQLite format 3\0")?;
        assert!(matches!(sniff(&db)?, NativeFileDto::Database { .. }));
        let sql = root.path().join("query.sql");
        std::fs::write(&sql, "DROP TABLE example;")?;
        let text = serde_json::to_value(sniff(&sql)?)?;
        assert_eq!(text["kind"], "sql");
        assert_eq!(text["opened"]["sql"], "DROP TABLE example;");
        std::fs::write(&sql, b"SELECT\0binary")?;
        assert!(sniff(&sql).is_err());
        std::fs::write(&sql, [0xff, 0xfe])?;
        assert!(sniff(&sql).is_err());
        std::fs::write(&db, "binary junk")?;
        assert!(sniff(&db).is_err());
        assert!(
            sniff(&root.path().join("data.duckdb"))
                .err()
                .is_some_and(|e| e.message.contains("M2-04"))
        );
        Ok(())
    }
    #[test]
    fn associations_cover_only_document_formats() -> Result<(), serde_json::Error> {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))?;
        let extensions = config["bundle"]["fileAssociations"]
            .as_array()
            .ok_or_else(|| serde_json::Error::io(std::io::Error::other("missing associations")))?;
        let names: Vec<_> = extensions
            .iter()
            .flat_map(|association| association["ext"].as_array().into_iter().flatten())
            .filter_map(serde_json::Value::as_str)
            .collect();
        assert_eq!(names, ["sqlite", "sqlite3", "db", "duckdb", "sql"]);
        Ok(())
    }
}
