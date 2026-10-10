// SPDX-License-Identifier: Apache-2.0
//! `.sql` files of query tabs: Run ▸ Save SQL As… (native save panel), ⌘S
//! on a tab bound to a file, and Open SQL File… into a new tab. Native
//! panels pick the paths; the webview never gets file-system access.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specta::Type;
use tauri_plugin_dialog::{DialogExt as _, FileDialogBuilder, FilePath};

use crate::connections::{IpcError, IpcErrorKind};

/// Largest `.sql` file opened into a tab.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// A `.sql` file a tab is bound to; `name` is its file name and
/// `modifiedMs` its modification time when it was read or written, so a
/// later change by another program can be noticed.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SqlFileDto {
    path: String,
    name: String,
    modified_ms: Option<f64>,
}

/// An opened `.sql` file and its text.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenedSqlFileDto {
    file: SqlFileDto,
    sql: String,
}

impl OpenedSqlFileDto {
    /// The file at `path` and its text.
    pub(crate) fn read(path: &Path) -> Result<Self, IpcError> {
        let sql = read_sql(path)?;
        Ok(Self {
            file: SqlFileDto::of(path),
            sql,
        })
    }
}

impl SqlFileDto {
    fn of(path: &Path) -> Self {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        Self {
            path: path.display().to_string(),
            name,
            modified_ms: super::file_watch::modified_ms(path),
        }
    }
}

fn io(path: &Path, error: &std::io::Error) -> IpcError {
    IpcError::new(
        IpcErrorKind::Storage,
        format!("{}: {error}", path.display()),
    )
}

fn is_sql(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"))
}

/// Show a panel attached to `window` and wait for the chosen path.
pub(crate) async fn choose(
    panel: FileDialogBuilder<tauri::Wry>,
    save: bool,
) -> Result<Option<PathBuf>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let reply = move |picked: Option<FilePath>| {
        // The receiver only disappears when the command was dropped.
        let _ = sender.send(picked);
    };
    if save {
        panel.save_file(reply);
    } else {
        panel.pick_file(reply);
    }
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The file panel closed unexpectedly"))?;
    picked
        .map(|file| {
            file.into_path()
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .transpose()
}

fn write_sql(path: &Path, sql: &str) -> Result<(), IpcError> {
    if !path.is_absolute() || !is_sql(path) {
        return Err(IpcError::invalid(
            "Only absolute .sql file paths can be saved",
        ));
    }
    write_atomic(path, sql.as_bytes())
}

/// Replace the file's contents atomically (temporary sibling, then rename),
/// following a symlink to the real file.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), IpcError> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut temporary = target.clone().into_os_string();
    temporary.push(".openrows-save");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, contents).map_err(|error| io(&temporary, &error))?;
    fs::rename(&temporary, &target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        io(&target, &error)
    })
}

fn read_sql(path: &Path) -> Result<String, IpcError> {
    let file = fs::File::open(path).map_err(|error| io(path, &error))?;
    let size = file.metadata().map_err(|error| io(path, &error))?.len();
    if size > MAX_FILE_BYTES {
        return Err(IpcError::invalid(
            "SQL files larger than 16 MiB cannot be opened",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(path, &error))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(IpcError::invalid(
            "SQL files larger than 16 MiB cannot be opened",
        ));
    }
    if bytes
        .iter()
        .any(|byte| *byte < 32 && !matches!(*byte, 9 | 10 | 13))
    {
        return Err(IpcError::invalid(
            "The SQL file contains binary control characters",
        ));
    }
    String::from_utf8(bytes).map_err(|_| IpcError::invalid("The file is not UTF-8 text"))
}

/// A file name for the save panel from a tab title, e.g. `SQL Query 1.sql`.
fn suggested_name(title: &str) -> String {
    let stem = file_stem(title.trim().strip_suffix(".sql").unwrap_or(title), "query");
    format!("{stem}.sql")
}

/// `title` without path separators for a save panel, or `fallback` when blank.
pub(crate) fn file_stem(title: &str, fallback: &str) -> String {
    let clean: String = title
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | ':' | '\\') {
                '-'
            } else {
                c
            }
        })
        .collect();
    if clean.is_empty() {
        fallback.to_owned()
    } else {
        clean
    }
}

/// Ask where to save `sql` and write it; `null` when the panel is cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sql_file_save_as(
    window: tauri::WebviewWindow,
    title: String,
    sql: String,
) -> Result<Option<SqlFileDto>, IpcError> {
    let panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Save SQL")
        .add_filter("SQL", &["sql"])
        .set_file_name(suggested_name(&title));
    let Some(mut path) = choose(panel, true).await? else {
        return Ok(None);
    };
    if !is_sql(&path) {
        path.as_mut_os_string().push(".sql");
    }
    write_sql(&path, &sql)?;
    Ok(Some(SqlFileDto::of(&path)))
}

/// Write a tab's SQL to the `.sql` file it is bound to; returns the file's
/// new modification time.
#[tauri::command]
#[specta::specta]
pub(crate) fn sql_file_save(path: String, sql: String) -> Result<Option<f64>, IpcError> {
    let path = Path::new(&path);
    write_sql(path, &sql)?;
    Ok(super::file_watch::modified_ms(path))
}

/// Pick a `.sql` file and read it; `null` when the panel is cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sql_file_open(
    window: tauri::WebviewWindow,
) -> Result<Option<OpenedSqlFileDto>, IpcError> {
    let panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Open SQL File")
        .add_filter("SQL", &["sql"]);
    let Some(path) = choose(panel, false).await? else {
        return Ok(None);
    };
    OpenedSqlFileDto::read(&path).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_names_end_in_sql_without_separators() {
        assert_eq!(suggested_name("SQL Query 1"), "SQL Query 1.sql");
        assert_eq!(suggested_name("a/b:c.sql"), "a-b-c.sql");
        assert_eq!(suggested_name("  "), "query.sql");
    }

    #[test]
    fn saving_replaces_the_file_and_reads_back() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.sql");
        write_sql(&path, "SELECT 1;")?;
        write_sql(&path, "SELECT 2;")?;
        assert_eq!(read_sql(&path)?, "SELECT 2;");
        assert_eq!(
            fs::read_dir(dir.path())?.count(),
            1,
            "no temporary file left"
        );
        assert_eq!(SqlFileDto::of(&path).name, "report.sql");
        Ok(())
    }

    #[test]
    fn only_absolute_sql_paths_are_written() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        assert!(write_sql(&dir.path().join("notes.txt"), "x").is_err());
        assert!(write_sql(Path::new("relative.sql"), "x").is_err());
        fs::write(dir.path().join("binary.sql"), [0xff, 0xfe])?;
        assert!(read_sql(&dir.path().join("binary.sql")).is_err());
        Ok(())
    }
}
