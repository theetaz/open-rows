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
    /// Horizontal tab (TSV).
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

/// The character that encloses fields needing quotes; it is doubled inside.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Quote {
    /// `"` (RFC 4180).
    #[default]
    Double,
    /// `'`.
    Single,
}

impl Quote {
    pub(crate) fn as_char(self) -> char {
        match self {
            Self::Double => '"',
            Self::Single => '\'',
        }
    }
}

/// Record separator of delimited text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineEnding {
    /// `\r\n` (RFC 4180, Excel).
    #[default]
    Crlf,
    /// `\n` (Unix tools).
    Lf,
}

impl LineEnding {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "\r\n",
            Self::Lf => "\n",
        }
    }
}

/// Character encoding of a text file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextEncoding {
    /// UTF-8 without a byte-order mark.
    #[default]
    Utf8,
    /// UTF-8 with a byte-order mark, which Excel needs to detect UTF-8.
    Utf8Bom,
    /// UTF-16 little endian with a byte-order mark.
    Utf16Le,
    /// ISO-8859-1; a character outside it fails the export.
    Latin1,
}

/// CSV and TSV options. The default is RFC 4180: `,`, `"` quoting with
/// doubled quotes, a header record, CRLF line ends and UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvOptions {
    /// Field separator.
    pub delimiter: Delimiter,
    /// Quote character.
    pub quote: Quote,
    /// Write the column names as the first record.
    pub header: bool,
    /// Text written for NULL; empty by default, so an empty string is
    /// quoted to stay distinguishable. A value equal to it is quoted too.
    pub null_text: String,
    /// Record separator.
    pub line_ending: LineEnding,
    /// File encoding.
    pub encoding: TextEncoding,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: Delimiter::Comma,
            quote: Quote::Double,
            header: true,
            null_text: String::new(),
            line_ending: LineEnding::Crlf,
            encoding: TextEncoding::Utf8,
        }
    }
}

impl CsvOptions {
    /// Tab-separated values with otherwise default options.
    #[must_use]
    pub fn tsv() -> Self {
        Self {
            delimiter: Delimiter::Tab,
            ..Self::default()
        }
    }
}

/// How JSON rows are laid out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonLayout {
    /// One JSON array of row objects, one compact object per line.
    Array,
    /// One JSON array of row objects, indented by two spaces.
    Pretty,
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

/// Excel workbook options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExcelOptions {
    /// Name of the first sheet; later tables name their own sheets.
    pub sheet: String,
    /// Write the column names as a bold first row of every sheet.
    pub header: bool,
}

/// Parquet column compression; both codecs are pure Rust.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParquetCompression {
    /// Snappy: fast, read by every Parquet tool.
    #[default]
    Snappy,
    /// No compression.
    Uncompressed,
}

/// What the export writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Format {
    /// Comma- (or otherwise) separated values, TSV included.
    Csv(CsvOptions),
    /// JSON objects keyed by column name.
    Json(JsonLayout),
    /// SQL `INSERT` statements.
    Insert(InsertOptions),
    /// A GitHub-flavoured Markdown table.
    Markdown,
    /// An Excel `.xlsx` workbook with typed cells.
    Excel(ExcelOptions),
    /// An Apache Parquet file with the result's Arrow types.
    Parquet(ParquetCompression),
}
