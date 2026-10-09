// SPDX-License-Identifier: Apache-2.0
//! Typed failures of tool discovery, command building and runs.
use std::io;
use std::path::{Path, PathBuf};

/// Why a backup or restore cannot be planned or run.
#[derive(Debug, thiserror::Error)]
pub enum DumpError {
    /// No installed copy of the tool was found.
    #[error("{tool} was not found: install it or set its path in Settings ▸ Tools")]
    NotFound {
        /// The tool's name, e.g. `pg_dump`.
        tool: &'static str,
    },
    /// A configured path is not an executable file.
    #[error("{} is not an executable file", .path.display())]
    NotExecutable {
        /// The configured path.
        path: PathBuf,
    },
    /// The program answered `--version` with something unexpected.
    #[error("{} is not {tool}: it reported {reported:?}", .path.display())]
    WrongTool {
        /// The program that was asked.
        path: PathBuf,
        /// The tool it should have been.
        tool: &'static str,
        /// The first line it printed.
        reported: String,
    },
    /// The options cannot produce a valid command.
    #[error("{0}")]
    Invalid(String),
    /// A file or process operation failed.
    #[error("{context}: {source}")]
    Io {
        /// What was being done, naming the path or program.
        context: String,
        /// The operating system error.
        #[source]
        source: io::Error,
    },
    /// A `SQLite` backup or restore failed.
    #[error("SQLite: {0}")]
    Sqlite(String),
}

impl DumpError {
    /// An I/O failure while doing `what` with `path`.
    pub(crate) fn io(what: &str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            context: format!("{what} {}", path.display()),
            source,
        }
    }

    /// Options that cannot be combined, or values out of range.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }
}

impl From<rusqlite::Error> for DumpError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error.to_string())
    }
}
