// SPDX-License-Identifier: Apache-2.0
//! The output formats and their options.
use openrows_sql::Dialect;
use std::num::NonZeroUsize;

/// CSV field separator; the closed set keeps the quoting rules sound.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Delimiter {
    /// `,` (RFC 4180).
    #[default]
    Comma,
    /// `;`, common where `,` is the decimal separator.
    Semicolon,
    /// Horizontal tab.
    Tab,
    /// `|`.
    Pipe,
}

impl Delimiter {
    pub(crate) fn as_char(self) -> char {
        match self {
            Self::Comma => ',',
            Self::Semicolon => ';',
            Self::Tab => '\t',
            Self::Pipe => '|',
        }
    }
}

/// CSV options (RFC 4180: CRLF line ends, `"` quoting with doubled quotes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CsvOptions {
    /// Field separator.
    pub delimiter: Delimiter,
    /// Write the column names as the first record.
    pub header: bool,
}

/// How JSON rows are laid out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonLayout {
    /// One JSON array of row objects.
    Array,
    /// Newline-delimited JSON: one row object per line.
    Lines,
}

/// SQL `INSERT` options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertOptions {
    /// Dialect for identifier quoting and literals.
    pub dialect: Dialect,
    /// Target table name, written as one quoted identifier.
    pub table: String,
    /// Rows per `INSERT … VALUES` statement.
    pub rows_per_statement: NonZeroUsize,
}

/// What the export writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Format {
    /// Comma- (or otherwise) separated values.
    Csv(CsvOptions),
    /// JSON objects keyed by column name.
    Json(JsonLayout),
    /// SQL `INSERT` statements.
    Insert(InsertOptions),
}
