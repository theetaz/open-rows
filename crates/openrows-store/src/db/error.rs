// SPDX-License-Identifier: Apache-2.0
//! Typed failures of the local `SQLite` store.

use std::path::PathBuf;

/// Every way opening, migrating or using the local store can fail.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// `SQLite` reported an error during `context`.
    #[error("local store {context} failed")]
    Sqlite {
        /// What the store was doing, e.g. `"open"` or `"migrate"`.
        context: &'static str,
        /// Underlying `SQLite` error.
        #[source]
        source: rusqlite::Error,
    },
    /// A file-system operation around the database failed.
    #[error("local store file operation on {path} failed")]
    Io {
        /// Path involved.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The database was written by a newer app; it is left untouched.
    #[error("local store schema v{found} is newer than supported v{supported}")]
    NewerSchema {
        /// Version recorded in the database.
        found: u32,
        /// Newest version this build knows.
        supported: u32,
    },
    /// A stored value is outside its allowed set.
    #[error("local store column {column} holds an unknown value")]
    Corrupt {
        /// Column that held the bad value.
        column: &'static str,
    },
    /// A thread panicked while holding the writer connection.
    #[error("local store writer lock poisoned")]
    Poisoned,
}

/// Build a `map_err` adapter tagging `SQLite` errors with `context`.
pub(crate) fn sqlite(context: &'static str) -> impl Fn(rusqlite::Error) -> StoreError {
    move |source| StoreError::Sqlite { context, source }
}
