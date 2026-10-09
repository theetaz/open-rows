// SPDX-License-Identifier: Apache-2.0
//! Requests, replies and events of the import commands.
use openrows_import::{
    BadRows, ConflictPolicy, FileFormat, ParseOptions, RowError, RunMode, Summary, TransactionMode,
    TypeCategory,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::connections::IpcError;
use crate::tables::TableTarget;

/// Layout of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ImportFormatDto {
    /// Delimited text (CSV, TSV, …).
    Delimited,
    /// One JSON array of objects.
    JsonArray,
    /// One JSON object per line.
    JsonLines,
}

/// Parse options; `import_preview` returns the detected ones.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportOptionsDto {
    pub(crate) format: ImportFormatDto,
    /// One ASCII character; `\t` for tab.
    pub(crate) delimiter: String,
    /// One ASCII character, or `null` to read quotes as text.
    pub(crate) quote: Option<String>,
    /// The first delimited record names the columns.
    pub(crate) header: bool,
    /// WHATWG label, e.g. `utf-8` or `windows-1252`.
    pub(crate) encoding: String,
    /// Empty fields are NULL.
    pub(crate) empty_as_null: bool,
}

fn ascii(text: &str, what: &str) -> Result<u8, IpcError> {
    match text.as_bytes() {
        [byte] if byte.is_ascii() && !matches!(byte, b'\r' | b'\n') => Ok(*byte),
        _ => Err(IpcError::invalid(format!(
            "The {what} must be one character"
        ))),
    }
}

impl ImportOptionsDto {
    pub(crate) fn to_options(&self) -> Result<ParseOptions, IpcError> {
        let delimiter = ascii(&self.delimiter, "delimiter")?;
        let quote = self
            .quote
            .as_deref()
            .map(|quote| ascii(quote, "quote"))
            .transpose()?;
        if quote == Some(delimiter) {
            return Err(IpcError::invalid("The quote and the delimiter must differ"));
        }
        Ok(ParseOptions {
            format: match self.format {
                ImportFormatDto::Delimited => FileFormat::Delimited,
                ImportFormatDto::JsonArray => FileFormat::JsonArray,
                ImportFormatDto::JsonLines => FileFormat::JsonLines,
            },
            delimiter,
            quote,
            header: self.header,
            encoding: self.encoding.clone(),
            empty_as_null: self.empty_as_null,
        })
    }
}

impl From<&ParseOptions> for ImportOptionsDto {
    fn from(options: &ParseOptions) -> Self {
        Self {
            format: match options.format {
                FileFormat::Delimited => ImportFormatDto::Delimited,
                FileFormat::JsonArray => ImportFormatDto::JsonArray,
                FileFormat::JsonLines => ImportFormatDto::JsonLines,
            },
            delimiter: char::from(options.delimiter).to_string(),
            quote: options.quote.map(|quote| char::from(quote).to_string()),
            header: options.header,
            encoding: options.encoding.clone(),
            empty_as_null: options.empty_as_null,
        }
    }
}

/// The kind of values a column holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TypeCategoryDto {
    Integer,
    Decimal,
    Boolean,
    Date,
    Timestamp,
    TimestampTz,
    Json,
    Text,
}

impl From<TypeCategory> for TypeCategoryDto {
    fn from(category: TypeCategory) -> Self {
        match category {
            TypeCategory::Integer => Self::Integer,
            TypeCategory::Decimal => Self::Decimal,
            TypeCategory::Boolean => Self::Boolean,
            TypeCategory::Date => Self::Date,
            TypeCategory::Timestamp => Self::Timestamp,
            TypeCategory::TimestampTz => Self::TimestampTz,
            TypeCategory::Json => Self::Json,
            TypeCategory::Text => Self::Text,
        }
    }
}

/// `picked` with a handle for the chosen file; `dismissed` when cancelled.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ImportFileReply {
    Picked {
        /// Names the file in later calls; the path stays in the core.
        file_id: String,
        file_name: String,
        size_bytes: f64,
    },
    Dismissed,
}

/// Preview a picked file with given (or detected) options.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPreviewRequest {
    pub(crate) file_id: String,
    /// The target connection, for type suggestions in its dialect.
    pub(crate) profile_id: String,
    /// `null` detects the options.
    pub(crate) options: Option<ImportOptionsDto>,
    /// Target column type per source column (`null`: not checked), for
    /// highlighting values the target would reject.
    pub(crate) types: Vec<Option<String>>,
}

/// One source column of the preview.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportColumnDto {
    pub(crate) name: String,
    pub(crate) category: TypeCategoryDto,
    pub(crate) nullable: bool,
    /// The type a new table gets for it in the connection's dialect.
    pub(crate) suggested_type: String,
}

/// A preview cell the target column would reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportCellDto {
    pub(crate) row: u32,
    pub(crate) column: u32,
}

/// A row that could not be imported.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RowErrorDto {
    /// Line (record number for JSON arrays).
    pub(crate) line: f64,
    pub(crate) column: Option<String>,
    pub(crate) message: String,
}

impl From<&RowError> for RowErrorDto {
    fn from(error: &RowError) -> Self {
        Self {
            line: count(error.line),
            column: error.column.clone(),
            message: error.message.clone(),
        }
    }
}

/// The file's columns with inferred types and its first rows.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPreviewDto {
    pub(crate) options: ImportOptionsDto,
    pub(crate) columns: Vec<ImportColumnDto>,
    pub(crate) rows: Vec<Vec<Option<String>>>,
    /// Line of each preview row.
    pub(crate) lines: Vec<f64>,
    pub(crate) invalid: Vec<ImportCellDto>,
    /// Malformed rows within the sample.
    pub(crate) errors: Vec<RowErrorDto>,
    /// Rows read for type inference.
    pub(crate) sampled: u32,
}

/// Where a target column's values come from.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ImportSourceDto {
    /// The source column at `index`.
    Column { index: u32 },
    /// The same value in every row; `null` is NULL.
    Constant { value: Option<String> },
}

/// One target column and its source.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportMappingDto {
    pub(crate) target: String,
    pub(crate) source: ImportSourceDto,
}

/// A new table: a type per mapped column and its primary key.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportCreateDto {
    /// Types in `columns` order, in the connection's dialect.
    pub(crate) types: Vec<String>,
    pub(crate) primary_key: Vec<String>,
}

macro_rules! choice {
    ($name:ident { $($variant:ident => $value:expr),+ $(,)? } -> $target:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
        #[serde(rename_all = "camelCase")]
        pub(crate) enum $name { $($variant),+ }

        impl From<$name> for $target {
            fn from(choice: $name) -> Self {
                match choice { $($name::$variant => $value),+ }
            }
        }
    };
}

choice!(ConflictDto { Error => ConflictPolicy::Error, Skip => ConflictPolicy::Skip, Update => ConflictPolicy::Update } -> ConflictPolicy);
choice!(TransactionDto { Single => TransactionMode::Single, Batches => TransactionMode::PerBatch } -> TransactionMode);
choice!(BadRowsDto { Abort => BadRows::Abort, Skip => BadRows::Skip } -> BadRows);
choice!(ImportModeDto { Check => RunMode::Check, Import => RunMode::Import } -> RunMode);

/// Start a dry run or an import of a picked file into a table.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRequest {
    /// An open session of the target database (proves it is connected).
    pub(crate) session_id: String,
    /// The table (existing, or the new table's name).
    pub(crate) target: TableTarget,
    pub(crate) file_id: String,
    pub(crate) options: ImportOptionsDto,
    pub(crate) columns: Vec<ImportMappingDto>,
    /// Set to create the table first.
    pub(crate) create: Option<ImportCreateDto>,
    /// Columns identifying existing rows (conflicts and the dry run).
    pub(crate) key: Vec<String>,
    pub(crate) conflict: ConflictDto,
    pub(crate) truncate: bool,
    pub(crate) transaction: TransactionDto,
    pub(crate) bad_rows: BadRowsDto,
    pub(crate) mode: ImportModeDto,
    /// Rows per batch (1–100 000).
    pub(crate) batch_size: u32,
}

/// The job started; events follow; `import_cancel(importId)` stops it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportReply {
    pub(crate) import_id: String,
}

/// Counts of a dry run or import.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportSummaryDto {
    /// Rows read, good and bad.
    pub(crate) rows: f64,
    /// Rows written and kept (inserted or updated).
    pub(crate) inserted: f64,
    pub(crate) skipped: f64,
    pub(crate) failed: f64,
    /// Dry run: rows whose key exists; `null` when it cannot be told.
    pub(crate) conflicts: Option<f64>,
    /// The first 100 row errors.
    pub(crate) errors: Vec<RowErrorDto>,
}

impl From<&Summary> for ImportSummaryDto {
    fn from(summary: &Summary) -> Self {
        Self {
            rows: count(summary.rows),
            inserted: count(summary.inserted),
            skipped: count(summary.skipped),
            failed: count(summary.failed),
            conflicts: summary.conflicts.map(count),
            errors: summary.errors.iter().map(RowErrorDto::from).collect(),
        }
    }
}

/// Progress, then exactly one of `finished`, `failed` or `cancelled`.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ImportEventDto {
    /// At most 10 per second.
    Progress {
        rows: f64,
        inserted: f64,
        failed: f64,
        /// Share of the file read, 0–1.
        fraction: f64,
        rows_per_second: f64,
    },
    Finished {
        summary: ImportSummaryDto,
    },
    /// Uncommitted work was rolled back; `summary.inserted` rows stayed.
    Failed {
        error: IpcError,
        summary: ImportSummaryDto,
    },
    Cancelled {
        summary: ImportSummaryDto,
    },
}

#[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^53")]
pub(crate) fn count(value: u64) -> f64 {
    value as f64
}
