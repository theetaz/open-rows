// SPDX-License-Identifier: Apache-2.0
//! How a file is parsed: its format, delimiter, quoting, header and
//! encoding (FR-IO-06). [`crate::inspect`] detects them when not given.

/// The file's layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileFormat {
    /// Delimited text: CSV, TSV and other separators (RFC 4180 quoting).
    Delimited,
    /// One JSON array of objects.
    JsonArray,
    /// One JSON object per line (NDJSON, JSON Lines).
    JsonLines,
}

/// Parse options of one file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseOptions {
    /// The file's layout.
    pub format: FileFormat,
    /// Field separator of delimited files (ASCII).
    pub delimiter: u8,
    /// Quote character of delimited files; `None` reads quotes as text.
    pub quote: Option<u8>,
    /// The first delimited record names the columns.
    pub header: bool,
    /// WHATWG encoding label, e.g. `utf-8`, `utf-16le` or `windows-1252`. A
    /// byte-order mark overrides it.
    pub encoding: String,
    /// Empty delimited fields (and empty JSON strings) are NULL.
    pub empty_as_null: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            format: FileFormat::Delimited,
            delimiter: b',',
            quote: Some(b'"'),
            header: true,
            encoding: "utf-8".to_owned(),
            empty_as_null: true,
        }
    }
}
