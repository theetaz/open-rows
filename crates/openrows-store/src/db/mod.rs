// SPDX-License-Identifier: Apache-2.0
//! The `SQLite` local store: one writer connection behind a mutex.

mod change_sets;
mod error;
mod history;
mod migrate;
mod saved_queries;

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use rusqlite::{Connection, OptionalExtension};

pub use change_sets::ChangeSetRecord;
pub use error::StoreError;
use error::sqlite;
pub use history::{HistoryEntry, HistoryRecord, HistorySource, HistoryStatus};
pub use migrate::SCHEMA_VERSION;
pub use saved_queries::{NewSavedQuery, SavedQueryRecord};

/// The app database (`openrows.sqlite`, docs/15 §2).
///
/// A single connection serialises writes (single-writer design); a reader
/// pool can be added when read concurrency is measured to matter.
#[derive(Debug)]
pub struct LocalStore {
    conn: Mutex<Connection>,
}

impl LocalStore {
    /// Open or create the database at `path`, enable WAL and apply pending
    /// migrations. An older database is first copied to
    /// `<dir>/backups/openrows.sqlite.v<N>`.
    ///
    /// # Errors
    /// [`StoreError::NewerSchema`] for a database from a newer app, or
    /// `Sqlite`/`Io` errors when the file cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path).map_err(sqlite("open"))?;
        configure(&conn)?;
        migrate::migrate(&mut conn, migrate::MIGRATIONS, &backup_dir(path))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// The schema version recorded in the database.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the pragma cannot be read.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        migrate::user_version(&*self.lock()?)
    }

    /// Read a `meta` value.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the query fails.
    pub fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.lock()?
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(sqlite("read meta"))
    }

    /// Insert or replace a `meta` value.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2) \
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map(drop)
            .map_err(sqlite("write meta"))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }
}

fn configure(conn: &Connection) -> Result<(), StoreError> {
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(sqlite("enable WAL"))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(sqlite("set synchronous"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(sqlite("enable foreign keys"))
}

fn backup_dir(path: &Path) -> PathBuf {
    path.parent()
        .map_or_else(|| PathBuf::from("backups"), |dir| dir.join("backups"))
}
