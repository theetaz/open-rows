// SPDX-License-Identifier: Apache-2.0
//! Export formats and their options as the webview sends them.
use crate::connections::IpcError;
use openrows_export::{
    CsvOptions, Delimiter, ExcelOptions, Format, InsertOptions, JsonLayout, LineEnding,
    ParquetCompression, Quote, TextEncoding,
};
use openrows_sql::Dialect;
use serde::Deserialize;
use specta::Type;
use std::num::NonZeroUsize;

/// Most rows one `INSERT … VALUES` may carry.
const MAX_ROWS_PER_INSERT: u32 = 10_000;
/// Longest NULL text, in characters.
const MAX_NULL_TEXT: usize = 32;

/// CSV field separator.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CsvDelimiterDto {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

/// Quote character of delimited text.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CsvQuoteDto {
    Double,
    Single,
}

/// Record separator of delimited text.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LineEndingDto {
    Crlf,
    Lf,
}

/// Encoding of delimited text.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TextEncodingDto {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Latin1,
}

/// Parquet compression codec.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ParquetCompressionDto {
    Snappy,
    None,
}

/// Options shared by CSV and TSV.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DelimitedDto {
    pub(crate) quote: CsvQuoteDto,
    pub(crate) header: bool,
    /// Text written for NULL; empty writes an empty field.
    pub(crate) null_text: String,
    pub(crate) line_ending: LineEndingDto,
    pub(crate) encoding: TextEncodingDto,
}

/// Output format and its options.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExportFormatDto {
    /// Comma- (or otherwise) separated values.
    Csv {
        delimiter: CsvDelimiterDto,
        options: DelimitedDto,
    },
    /// Tab-separated values.
    Tsv { options: DelimitedDto },
    /// An array of row objects, compact or indented.
    Json { pretty: bool },
    /// One row object per line.
    Ndjson,
    /// `INSERT` statements for `table`, `rowsPerStatement` rows each.
    Sql {
        table: String,
        rows_per_statement: u32,
    },
    /// A GitHub-flavoured Markdown table.
    Markdown,
    /// An Excel workbook with typed cells.
    Xlsx { header: bool },
    /// An Apache Parquet file.
    Parquet { compression: ParquetCompressionDto },
}

impl ExportFormatDto {
    /// The core format for one file named `name` (the Excel sheet), with
    /// literals and quoting for `dialect`.
    pub(crate) fn to_format(&self, dialect: Dialect, name: &str) -> Result<Format, IpcError> {
        Ok(match self {
            Self::Csv { delimiter, options } => {
                Format::Csv(delimited(delimiter_of(*delimiter), options)?)
            }
            Self::Tsv { options } => Format::Csv(delimited(Delimiter::Tab, options)?),
            Self::Json { pretty: false } => Format::Json(JsonLayout::Array),
            Self::Json { pretty: true } => Format::Json(JsonLayout::Pretty),
            Self::Ndjson => Format::Json(JsonLayout::Lines),
            Self::Sql {
                table,
                rows_per_statement,
            } => Format::Insert(insert_options(dialect, table, *rows_per_statement)?),
            Self::Markdown => Format::Markdown,
            Self::Xlsx { header } => Format::Excel(ExcelOptions {
                sheet: name.to_owned(),
                header: *header,
            }),
            Self::Parquet { compression } => Format::Parquet(match compression {
                ParquetCompressionDto::Snappy => ParquetCompression::Snappy,
                ParquetCompressionDto::None => ParquetCompression::Uncompressed,
            }),
        })
    }

    /// The same options for the table `table` (SQL `INSERT`s target it).
    pub(crate) fn for_table(&self, table: &str) -> Self {
        match self {
            Self::Sql {
                rows_per_statement, ..
            } => Self::Sql {
                table: table.to_owned(),
                rows_per_statement: *rows_per_statement,
            },
            other => other.clone(),
        }
    }

    /// File extension and save-panel filter label.
    pub(crate) fn extension(&self) -> (&'static str, &'static str) {
        match self {
            Self::Csv { .. } => ("csv", "CSV"),
            Self::Tsv { .. } => ("tsv", "TSV"),
            Self::Json { .. } => ("json", "JSON"),
            Self::Ndjson => ("ndjson", "NDJSON"),
            Self::Sql { .. } => ("sql", "SQL"),
            Self::Markdown => ("md", "Markdown"),
            Self::Xlsx { .. } => ("xlsx", "Excel workbook"),
            Self::Parquet { .. } => ("parquet", "Parquet"),
        }
    }
}

fn delimiter_of(delimiter: CsvDelimiterDto) -> Delimiter {
    match delimiter {
        CsvDelimiterDto::Comma => Delimiter::Comma,
        CsvDelimiterDto::Semicolon => Delimiter::Semicolon,
        CsvDelimiterDto::Tab => Delimiter::Tab,
        CsvDelimiterDto::Pipe => Delimiter::Pipe,
    }
}

fn delimited(delimiter: Delimiter, options: &DelimitedDto) -> Result<CsvOptions, IpcError> {
    let (quote, quote_char) = match options.quote {
        CsvQuoteDto::Double => (Quote::Double, '"'),
        CsvQuoteDto::Single => (Quote::Single, '\''),
    };
    let clashes =
        |c: char| c == quote_char || matches!(c, '\r' | '\n') || c == delimiter_char(delimiter);
    if options.null_text.chars().count() > MAX_NULL_TEXT || options.null_text.contains(clashes) {
        return Err(IpcError::invalid(
            "The NULL text must be short and cannot contain the delimiter, the quote or a line break",
        ));
    }
    Ok(CsvOptions {
        delimiter,
        quote,
        header: options.header,
        null_text: options.null_text.clone(),
        line_ending: match options.line_ending {
            LineEndingDto::Crlf => LineEnding::Crlf,
            LineEndingDto::Lf => LineEnding::Lf,
        },
        encoding: match options.encoding {
            TextEncodingDto::Utf8 => TextEncoding::Utf8,
            TextEncodingDto::Utf8Bom => TextEncoding::Utf8Bom,
            TextEncodingDto::Utf16Le => TextEncoding::Utf16Le,
            TextEncodingDto::Latin1 => TextEncoding::Latin1,
        },
    })
}

fn delimiter_char(delimiter: Delimiter) -> char {
    match delimiter {
        Delimiter::Comma => ',',
        Delimiter::Semicolon => ';',
        Delimiter::Tab => '\t',
        Delimiter::Pipe => '|',
    }
}

fn insert_options(dialect: Dialect, table: &str, rows: u32) -> Result<InsertOptions, IpcError> {
    let table = table.trim();
    if table.is_empty() || table.contains('\0') {
        return Err(IpcError::invalid(
            "Enter the table name the INSERT statements target",
        ));
    }
    let rows = usize::try_from(rows.min(MAX_ROWS_PER_INSERT))
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| IpcError::invalid("Rows per INSERT must be at least 1"))?;
    Ok(InsertOptions {
        dialect,
        table: table.to_owned(),
        rows_per_statement: rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delimited_options(null_text: &str) -> DelimitedDto {
        DelimitedDto {
            quote: CsvQuoteDto::Double,
            header: true,
            null_text: null_text.to_owned(),
            line_ending: LineEndingDto::Lf,
            encoding: TextEncodingDto::Utf8Bom,
        }
    }

    #[test]
    fn formats_map_to_core_options_and_validate() -> Result<(), IpcError> {
        let sql = ExportFormatDto::Sql {
            table: " users ".to_owned(),
            rows_per_statement: 50_000,
        };
        let Format::Insert(options) = sql.to_format(Dialect::MySql, "x")? else {
            return Err(IpcError::invalid("not an insert"));
        };
        assert_eq!(options.table, "users");
        assert_eq!(options.rows_per_statement.get(), 10_000);
        let Format::Insert(options) = sql.for_table("orders").to_format(Dialect::MySql, "x")?
        else {
            return Err(IpcError::invalid("not an insert"));
        };
        assert_eq!(options.table, "orders");
        let zero = ExportFormatDto::Sql {
            table: "t".to_owned(),
            rows_per_statement: 0,
        };
        assert!(zero.to_format(Dialect::Postgres, "x").is_err());
        let tsv = ExportFormatDto::Tsv {
            options: delimited_options("NULL"),
        };
        let Format::Csv(options) = tsv.to_format(Dialect::Sqlite, "x")? else {
            return Err(IpcError::invalid("not delimited"));
        };
        assert_eq!(options.delimiter, Delimiter::Tab);
        assert_eq!(options.encoding, TextEncoding::Utf8Bom);
        assert_eq!(tsv.extension(), ("tsv", "TSV"));
        let clash = ExportFormatDto::Csv {
            delimiter: CsvDelimiterDto::Comma,
            options: delimited_options("a,b"),
        };
        assert!(clash.to_format(Dialect::Sqlite, "x").is_err());
        let workbook = ExportFormatDto::Xlsx { header: true };
        assert_eq!(
            workbook.to_format(Dialect::Sqlite, "orders")?,
            Format::Excel(ExcelOptions {
                sheet: "orders".to_owned(),
                header: true
            })
        );
        assert_eq!(
            ExportFormatDto::Ndjson.to_format(Dialect::Sqlite, "x")?,
            Format::Json(JsonLayout::Lines)
        );
        Ok(())
    }
}
