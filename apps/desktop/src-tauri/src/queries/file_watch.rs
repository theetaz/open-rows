// SPDX-License-Identifier: Apache-2.0
//! Changes made to a tab's `.sql` file by another program (M1-21): the
//! webview compares the file's modification time with the one it last read
//! or wrote (on window focus) and offers Reload or Keep, as `TablePlus` and
//! VS Code do. Only absolute `.sql` paths are read.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::path::Path;
use std::time::UNIX_EPOCH;

use super::files::OpenedSqlFileDto;
use crate::connections::IpcError;

/// Modification time in Unix epoch milliseconds; `None` when the file is
/// gone or the platform does not record it.
#[expect(
    clippy::cast_precision_loss,
    reason = "epoch milliseconds stay far below 2^53"
)]
pub(crate) fn modified_ms(path: &Path) -> Option<f64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(elapsed.as_micros() as f64 / 1000.0)
}

fn checked(path: &str) -> Result<&Path, IpcError> {
    let path = Path::new(path);
    let sql = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"));
    if path.is_absolute() && sql {
        Ok(path)
    } else {
        Err(IpcError::invalid(
            "Only absolute .sql file paths can be read",
        ))
    }
}

/// The modification time of a tab's `.sql` file; `null` when it is gone.
#[tauri::command]
#[specta::specta]
pub(crate) fn sql_file_stat(path: String) -> Result<Option<f64>, IpcError> {
    Ok(modified_ms(checked(&path)?))
}

/// Read a tab's `.sql` file again (Reload after an outside change).
#[tauri::command]
#[specta::specta]
pub(crate) fn sql_file_read(path: String) -> Result<OpenedSqlFileDto, IpcError> {
    OpenedSqlFileDto::read(checked(&path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outside_write_changes_the_stamp_and_reads_back() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("report.sql");
        let text = path.display().to_string();
        assert_eq!(sql_file_stat(text.clone())?, None);
        std::fs::write(&path, "SELECT 1;")?;
        let first = sql_file_stat(text.clone())?.ok_or("no stamp")?;
        let file = std::fs::File::options().write(true).open(&path)?;
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1))?;
        assert_eq!(sql_file_stat(text.clone())?, Some(1000.0));
        assert!((first - 1000.0).abs() > f64::EPSILON);
        assert!(sql_file_stat("relative.sql".to_owned()).is_err());
        assert!(sql_file_stat(dir.path().join("x.txt").display().to_string()).is_err());
        let opened = serde_json::to_value(sql_file_read(text)?)?;
        assert_eq!(opened["sql"], "SELECT 1;");
        assert_eq!(opened["file"]["modifiedMs"], 1000.0);
        Ok(())
    }
}
