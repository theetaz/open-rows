// SPDX-License-Identifier: Apache-2.0
//! Typed import failures: whole-file problems ([`ImportError`]) and single
//! rows the import skips or stops at ([`RowError`]).
use std::fmt;

/// Why a file cannot be read or an import cannot be planned.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Opening or reading the file failed.
    #[error("the file could not be read: {0}")]
    Io(#[from] std::io::Error),
    /// The encoding label is not a known text encoding.
    #[error("{0:?} is not a known text encoding")]
    UnknownEncoding(String),
    /// A JSON array file is not valid JSON; the rest cannot be read.
    #[error("the JSON file is invalid at line {line}, column {column}: {message}")]
    Json {
        /// 1-based line of the problem.
        line: usize,
        /// 1-based column of the problem.
        column: usize,
        /// What the parser expected.
        message: String,
    },
    /// The file holds no columns to import.
    #[error("the file has no columns")]
    NoColumns,
    /// The mapping, target or options cannot produce valid SQL.
    #[error("{0}")]
    Invalid(String),
}

impl From<serde_json::Error> for ImportError {
    fn from(error: serde_json::Error) -> Self {
        if error.is_io() {
            return Self::Io(error.into());
        }
        let message = error.to_string();
        // serde_json appends " at line L column C"; keep only the cause.
        let message = message
            .split(" at line ")
            .next()
            .unwrap_or_default()
            .to_owned();
        Self::Json {
            line: error.line(),
            column: error.column(),
            message,
        }
    }
}

/// A row that could not be imported, by its line in the file (the record
/// number for a JSON array file).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowError {
    /// 1-based line (delimited and NDJSON files) or record number (JSON arrays).
    pub line: u64,
    /// The target column the value was meant for, when one value was at fault.
    pub column: Option<String>,
    /// What is wrong, without the full row.
    pub message: String,
}

impl RowError {
    /// An error about the whole row.
    #[must_use]
    pub fn row(line: u64, message: impl Into<String>) -> Self {
        Self {
            line,
            column: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.column {
            Some(column) => write!(f, "line {}, {column}: {}", self.line, self.message),
            None => write!(f, "line {}: {}", self.line, self.message),
        }
    }
}
