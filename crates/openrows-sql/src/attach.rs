// SPDX-License-Identifier: Apache-2.0
//! `SQLite` `ATTACH DATABASE` and `DETACH DATABASE` statements.
//!
//! The file is bound as a parameter in `SQLite`'s URI form
//! (`file:/path?mode=ro|rw&vfs=unix`), so an attached file is opened
//! read-only when the main database is, and is never created when it is
//! missing. URI filenames are honoured because the driver opens every
//! connection with `SQLITE_OPEN_URI`. The VFS is named because an in-memory
//! main database (`vfs=memdb`) would otherwise make `ATTACH` open an empty
//! memory database of that name. The alias is a quoted identifier.
use std::path::{MAIN_SEPARATOR, Path};

use crate::Dialect;
use crate::select::{BoundSql, SelectError};

/// The platform's default disk VFS of the bundled `SQLite`.
const DISK_VFS: &str = if cfg!(windows) { "win32" } else { "unix" };

/// `ATTACH DATABASE ?1 AS "alias"` with the file's URI bound.
///
/// # Errors
/// [`SelectError::NulCharacter`] for a NUL in the alias or path, and
/// [`SelectError::Unsupported`] for a path that is not UTF-8.
pub fn attach_sql(alias: &str, path: &Path, read_only: bool) -> Result<BoundSql, SelectError> {
    let dialect = Dialect::Sqlite;
    let uri = file_uri(path, read_only)?;
    Ok(BoundSql {
        sql: format!(
            "ATTACH DATABASE {} AS {}",
            dialect.placeholder(1),
            dialect.quote_ident(alias)?
        ),
        params: vec![Some(uri)],
    })
}

/// `DETACH DATABASE "alias"`.
///
/// # Errors
/// [`SelectError::NulCharacter`] for a NUL in the alias.
pub fn detach_sql(alias: &str) -> Result<BoundSql, SelectError> {
    Ok(BoundSql {
        sql: format!("DETACH DATABASE {}", Dialect::Sqlite.quote_ident(alias)?),
        params: Vec::new(),
    })
}

/// `file:/abs/path?mode=ro` (or `rw`) on the disk VFS: `%`, `?` and `#`
/// are escaped so the path cannot end early or add query parameters.
fn file_uri(path: &Path, read_only: bool) -> Result<String, SelectError> {
    let text = path.to_str().ok_or(SelectError::Unsupported(
        "database file paths must be valid UTF-8",
    ))?;
    if text.contains('\0') {
        return Err(SelectError::NulCharacter);
    }
    let mut uri = String::from("file:");
    if !text.starts_with(MAIN_SEPARATOR) && !text.starts_with('/') {
        // Windows drive paths become `file:/C:/…`.
        uri.push('/');
    }
    for character in text.chars() {
        match character {
            '%' => uri.push_str("%25"),
            '?' => uri.push_str("%3F"),
            '#' => uri.push_str("%23"),
            '\\' if MAIN_SEPARATOR == '\\' => uri.push('/'),
            other => uri.push(other),
        }
    }
    let mode = if read_only { "ro" } else { "rw" };
    Ok(format!("{uri}?mode={mode}&vfs={DISK_VFS}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_binds_the_file_uri_and_quotes_the_alias() -> Result<(), SelectError> {
        let bound = attach_sql("my \"db\"", Path::new("/data/a b?#%.db"), false)?;
        assert_eq!(bound.sql, "ATTACH DATABASE ?1 AS \"my \"\"db\"\"\"");
        assert_eq!(
            bound.params,
            vec![Some(format!(
                "file:/data/a b%3F%23%25.db?mode=rw&vfs={DISK_VFS}"
            ))]
        );
        let read_only = attach_sql("logs", Path::new("/data/logs.db"), true)?;
        assert_eq!(
            read_only.params,
            vec![Some(format!("file:/data/logs.db?mode=ro&vfs={DISK_VFS}"))]
        );
        Ok(())
    }

    #[test]
    fn detach_quotes_the_alias_and_nul_is_refused() -> Result<(), SelectError> {
        assert_eq!(detach_sql("logs")?.sql, "DETACH DATABASE \"logs\"");
        assert_eq!(detach_sql("a\0b"), Err(SelectError::NulCharacter));
        assert_eq!(
            attach_sql("logs", Path::new("/data/a\0.db"), false),
            Err(SelectError::NulCharacter)
        );
        Ok(())
    }
}
