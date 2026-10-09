// SPDX-License-Identifier: Apache-2.0
//! `SQLite` needs no external tool: a backup is `VACUUM INTO` a new file (a
//! consistent, compacted copy while others keep reading), and a restore
//! copies a backup over the live file through the online backup API, which
//! takes the proper locks instead of overwriting the file underneath open
//! connections.
use std::path::Path;
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::{Connection, OpenFlags};

use crate::error::DumpError;

/// Pages copied per backup step.
const PAGES_PER_STEP: std::ffi::c_int = 1024;
/// Wait before retrying a step while another connection writes.
const BUSY_PAUSE: Duration = Duration::from_millis(25);

/// Write a copy of the database at `source` to `destination`, which must not
/// exist yet. Returns the copy's size in bytes.
///
/// # Errors
/// The source cannot be opened or the copy cannot be written.
pub fn sqlite_backup(source: &Path, destination: &Path) -> Result<u64, DumpError> {
    let connection = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let target = destination
        .to_str()
        .ok_or_else(|| DumpError::invalid("The backup path is not valid UTF-8"))?;
    connection.execute("VACUUM INTO ?1", [target])?;
    std::fs::metadata(destination)
        .map(|meta| meta.len())
        .map_err(|error| DumpError::io("cannot read", destination, error))
}

/// Replace the contents of the database at `destination` with the backup
/// at `backup`, page by page under a write lock.
///
/// # Errors
/// The backup is not a `SQLite` database, or the destination cannot be written.
pub fn sqlite_restore(backup: &Path, destination: &Path) -> Result<(), DumpError> {
    let source = Connection::open_with_flags(
        backup,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // Fails with "file is not a database" before anything is overwritten.
    source.query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
        row.get::<_, i64>(0)
    })?;
    let mut target = Connection::open_with_flags(
        destination,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Backup::new(&source, &mut target)?.run_to_completion(PAGES_PER_STEP, BUSY_PAUSE, None)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/sqlite.rs"]
mod tests;
