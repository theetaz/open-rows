// SPDX-License-Identifier: Apache-2.0
//! Where a file engine (`SQLite`) finds its main database and which other
//! database files it attaches right after opening (docs/05 §13).
use std::path::PathBuf;

/// The main database of a file engine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileSource {
    /// A database file on disk.
    Disk {
        /// Absolute file path.
        path: PathBuf,
        /// Open read-only: the engine refuses every write (attached files too).
        read_only: bool,
        /// Create the file when it does not exist.
        create: bool,
    },
    /// An in-memory database shared by every connection in this process that
    /// opens the same `name`; it lives while at least one of them is open.
    Memory {
        /// Shared name: ASCII letters, digits, `-` and `_` only.
        name: String,
    },
}

impl FileSource {
    /// True when the engine must refuse writes to every database.
    #[must_use]
    pub fn read_only(&self) -> bool {
        matches!(
            self,
            Self::Disk {
                read_only: true,
                ..
            }
        )
    }

    /// The file path, or `:memory:`, for messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Disk { path, .. } => path.display().to_string(),
            Self::Memory { .. } => ":memory:".to_owned(),
        }
    }
}

/// Another database file attached to every connection under `alias`
/// (`ATTACH DATABASE … AS alias`); its tables appear as schema `alias`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    /// Schema name of the attached database.
    pub alias: String,
    /// Absolute path of an existing database file (never created).
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_writable_and_described_like_sqlite() {
        let memory = FileSource::Memory {
            name: "openrows-1".to_owned(),
        };
        assert!(!memory.read_only());
        assert_eq!(memory.describe(), ":memory:");
        let disk = FileSource::Disk {
            path: PathBuf::from("/data/app.db"),
            read_only: true,
            create: false,
        };
        assert!(disk.read_only());
        assert_eq!(disk.describe(), "/data/app.db");
    }
}
