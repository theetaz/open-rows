// SPDX-License-Identifier: Apache-2.0
//! Native document and `openrows://` link handoff, queued until the webview is
//! ready; never runs SQL. Links are parsed here, so the webview only sees
//! validated requests and rejected links are logged without their contents.
use crate::connections::{Connections, EngineDto, IpcError, IpcErrorKind};
use crate::links::{self, OpenLinkDto, ParsedLink};
use crate::queries::files::OpenedSqlFileDto;
use serde::Serialize;
use specta::Type;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use tauri::{Emitter, Manager, State};

/// Most handoffs waiting at once; later ones are dropped.
const MAX_PENDING: usize = 32;

/// One OS handoff awaiting the webview (never an execution request).
#[derive(Clone, Debug, PartialEq)]
enum Handoff {
    Path(PathBuf),
    Link(OpenLinkDto),
    Rejected(String),
}

/// Bounded handoffs awaiting the frontend.
#[derive(Default)]
pub(crate) struct NativeFiles(Mutex<VecDeque<Handoff>>);

/// Validated content or an actionable rejection, never executable instructions.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum NativeFileDto {
    Sql {
        opened: OpenedSqlFileDto,
    },
    Database {
        path: String,
        engine: EngineDto,
    },
    /// An `openrows://` request the user confirms before anything connects.
    Link {
        link: OpenLinkDto,
    },
    Rejected {
        message: String,
    },
}

/// Queue OS-selected paths and announce readiness without exposing file contents.
#[cfg(not(target_os = "macos"))]
fn enqueue(app: &tauri::AppHandle, paths: impl IntoIterator<Item = PathBuf>) {
    push(app, paths.into_iter().map(Handoff::Path));
}

/// Route URLs the OS opened the app with: local files and `openrows://` links.
pub(crate) fn opened(app: &tauri::AppHandle, urls: &[tauri::Url]) {
    push(app, urls.iter().filter_map(handoff));
}

fn handoff(url: &tauri::Url) -> Option<Handoff> {
    if url.scheme() == "file" {
        return url.to_file_path().ok().map(Handoff::Path);
    }
    if url.scheme() != links::SCHEME {
        return None;
    }
    Some(match links::parse_url(url) {
        Ok(ParsedLink::Open(link)) => Handoff::Link(link),
        Ok(ParsedLink::File(path)) => Handoff::Path(path),
        Err(error) => {
            tracing::warn!(reason = %error, "Rejected an openrows link");
            Handoff::Rejected(error.to_string())
        }
    })
}

fn push(app: &tauri::AppHandle, handoffs: impl IntoIterator<Item = Handoff>) {
    let state = app.state::<NativeFiles>();
    {
        let mut pending = state.0.lock().unwrap_or_else(PoisonError::into_inner);
        for handoff in handoffs {
            if pending.len() < MAX_PENDING && !pending.contains(&handoff) {
                pending.push_back(handoff);
            }
        }
    }
    let target = crate::windows::ensure_window(app);
    let _ = app.emit_to(target, "native-files-ready", ());
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

/// Drain queued handoffs into the window that should receive them (the
/// focused one); other windows get nothing. The lock guard applies at IPC.
#[tauri::command]
#[specta::specta]
pub(crate) async fn native_files_take(
    window: tauri::WebviewWindow,
    files: State<'_, NativeFiles>,
) -> Result<Vec<NativeFileDto>, IpcError> {
    if !crate::windows::is_target(&window) {
        return Ok(Vec::new());
    }
    let handoffs: Vec<_> = files
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect();
    tauri::async_runtime::spawn_blocking(move || handoffs.into_iter().map(resolve).collect())
        .await
        .map_err(|e| IpcError::new(IpcErrorKind::Internal, e.to_string()))
}

fn resolve(handoff: Handoff) -> NativeFileDto {
    match handoff {
        Handoff::Path(path) => sniff(&path).unwrap_or_else(|error| NativeFileDto::Rejected {
            message: error.message,
        }),
        Handoff::Link(link) => NativeFileDto::Link { link },
        Handoff::Rejected(message) => NativeFileDto::Rejected { message },
    }
}

fn sniff(path: &Path) -> Result<NativeFileDto, IpcError> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();
    if !path.is_absolute() || !path.is_file() {
        return Err(IpcError::invalid("Choose an existing local file"));
    }
    if extension == "sql" {
        return OpenedSqlFileDto::read(path).map(|opened| NativeFileDto::Sql { opened });
    }
    if !["sqlite", "sqlite3", "db", "duckdb"].contains(&extension.as_str()) {
        return Err(IpcError::invalid(
            "Unexpected file type; use SQLite, DuckDB or UTF-8 .sql",
        ));
    }
    let mut header = [0; 16];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|_| IpcError::invalid("This file has no valid database header"))?;
    let engine = match extension.as_str() {
        "duckdb" if &header[8..12] == b"DUCK" => EngineDto::Duckdb,
        "sqlite" | "sqlite3" | "db" if &header == b"SQLite format 3\0" => EngineDto::Sqlite,
        _ => {
            return Err(IpcError::invalid(
                "Database header does not match its file type",
            ));
        }
    };
    Ok(NativeFileDto::Database {
        path: path.display().to_string(),
        engine,
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
        let duckdb = root.path().join("data.duckdb");
        std::fs::write(&duckdb, b"12345678DUCK1234")?;
        assert!(matches!(
            sniff(&duckdb)?,
            NativeFileDto::Database {
                engine: EngineDto::Duckdb,
                ..
            }
        ));
        std::fs::write(&duckdb, b"SQLite format 3\0")?;
        assert!(sniff(&duckdb).is_err());
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
