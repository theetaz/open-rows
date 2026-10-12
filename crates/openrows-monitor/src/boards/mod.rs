// SPDX-License-Identifier: Apache-2.0
//! Analytics boards (M25-06): validated portable documents, read-only
//! admission, typed parameter binding and bounded result collection.
mod bind;
mod collect;
mod model;
#[cfg(test)]
mod tests;
mod validate;

pub use bind::{admit_sql, bind_widget, valid_value};
pub use collect::{BoardColumn, BoardResult, Collector, ColumnKind, Truncation};
pub use model::*;
pub use validate::{MAX_BOARD_BYTES, MAX_SQL, REFRESH_SECS, export_board, parse_board, validate};

/// Why a board document, statement or value was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoardError {
    /// The document exceeds the import limit, in bytes.
    #[error("Board files are limited to {0} bytes")]
    TooLarge(usize),
    /// Not a board document.
    #[error("Not a valid board file: {0}")]
    Malformed(String),
    /// A document version this build does not read.
    #[error("Unsupported board version {0}")]
    Version(u32),
    /// A field breaks a board rule.
    #[error("{0}")]
    Invalid(String),
    /// The widget SQL is not a single read.
    #[error("{0}")]
    Sql(String),
    /// A parameter value is missing or mistyped.
    #[error("{0}")]
    Param(String),
}
