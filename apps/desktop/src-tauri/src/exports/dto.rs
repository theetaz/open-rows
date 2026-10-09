// SPDX-License-Identifier: Apache-2.0
//! Request, reply and event types of `export_start`.
use crate::connections::{IpcError, IpcErrorKind};
use crate::tables::TableTarget;
use openrows_core::export::{ExportFailure, ExportOutcome};
use openrows_export::{CsvOptions, Delimiter, ExportError, Format, InsertOptions, JsonLayout};
use openrows_sql::Dialect;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::num::NonZeroUsize;

/// Most rows one `INSERT … VALUES` may carry.
const MAX_ROWS_PER_INSERT: u32 = 10_000;

/// Which rows to export.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExportSourceDto {
    /// A result held in memory (the rows a query tab received).
    Result {
        session_id: String,
        result_id: String,
    },
    /// Run one read-only statement again and stream all of its rows (a
    /// result the row limit cut short).
    Query { session_id: String, sql: String },
    /// Every row of a table or view.
    Table {
        session_id: String,
        target: TableTarget,
    },
}

impl ExportSourceDto {
    pub(crate) fn session_id(&self) -> &str {
        match self {
            Self::Result { session_id, .. }
            | Self::Query { session_id, .. }
            | Self::Table { session_id, .. } => session_id,
        }
    }
}

/// CSV field separator.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CsvDelimiterDto {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

/// Output format and its options.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExportFormatDto {
    /// RFC 4180 CSV.
    Csv {
        delimiter: CsvDelimiterDto,
        header: bool,
    },
    /// An array of row objects, or one object per line (NDJSON).
    Json { lines: bool },
    /// `INSERT` statements for `table`, `rowsPerStatement` rows each.
    Sql {
        table: String,
        rows_per_statement: u32,
    },
}

impl ExportFormatDto {
    /// The core format, with literals and quoting for `dialect`.
    pub(crate) fn to_format(&self, dialect: Dialect) -> Result<Format, IpcError> {
        Ok(match self {
            Self::Csv { delimiter, header } => Format::Csv(CsvOptions {
                delimiter: match delimiter {
                    CsvDelimiterDto::Comma => Delimiter::Comma,
                    CsvDelimiterDto::Semicolon => Delimiter::Semicolon,
                    CsvDelimiterDto::Tab => Delimiter::Tab,
                    CsvDelimiterDto::Pipe => Delimiter::Pipe,
                },
                header: *header,
            }),
            Self::Json { lines } => Format::Json(if *lines {
                JsonLayout::Lines
            } else {
                JsonLayout::Array
            }),
            Self::Sql {
                table,
                rows_per_statement,
            } => Format::Insert(insert_options(dialect, table, *rows_per_statement)?),
        })
    }

    /// File extension and save-panel filter label.
    pub(crate) fn extension(&self) -> (&'static str, &'static str) {
        match self {
            Self::Csv { .. } => ("csv", "CSV"),
            Self::Json { lines: false } => ("json", "JSON"),
            Self::Json { lines: true } => ("ndjson", "NDJSON"),
            Self::Sql { .. } => ("sql", "SQL"),
        }
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

/// Start an export; the native save panel picks the file.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportRequest {
    pub(crate) source: ExportSourceDto,
    pub(crate) format: ExportFormatDto,
    /// Suggested file name without extension, e.g. the table name.
    pub(crate) file_name: String,
}

/// `started` once the file was chosen; `dismissed` when the panel was cancelled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExportReply {
    /// Events follow on the channel; `query_cancel(exportId)` stops it.
    Started {
        export_id: String,
        file_name: String,
    },
    Dismissed,
}

/// Export progress; exactly one of `finished`, `failed` or `cancelled` ends it.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExportEventDto {
    /// Rows written so far; at most 10 per second.
    Progress {
        rows: f64,
    },
    /// The file is complete at `path`.
    Finished {
        rows: f64,
        path: String,
    },
    Failed {
        error: IpcError,
    },
    Cancelled,
}

#[expect(clippy::cast_precision_loss, reason = "row counts stay far below 2^53")]
pub(crate) fn rows(count: u64) -> f64 {
    count as f64
}

/// The terminal event for an export written to `path`.
pub(crate) fn terminal(outcome: ExportOutcome, path: String) -> ExportEventDto {
    match outcome {
        ExportOutcome::Written { rows: count } => ExportEventDto::Finished {
            rows: rows(count),
            path,
        },
        ExportOutcome::Cancelled => ExportEventDto::Cancelled,
        ExportOutcome::Failed(ExportFailure::Query(error)) => ExportEventDto::Failed {
            error: error.into(),
        },
        ExportOutcome::Failed(ExportFailure::Write(error)) => ExportEventDto::Failed {
            error: write_error(&error),
        },
    }
}

/// A file failure as a user-facing error.
pub(crate) fn write_error(error: &ExportError) -> IpcError {
    let kind = match error {
        ExportError::Io(_) | ExportError::InvalidPath => IpcErrorKind::Storage,
        ExportError::UnsupportedType { .. } => IpcErrorKind::Unsupported,
        ExportError::Aborted => IpcErrorKind::Cancelled,
        ExportError::InvalidName(_)
        | ExportError::NoColumns
        | ExportError::NoResult
        | ExportError::SchemaMismatch => IpcErrorKind::Invalid,
    };
    let message = error.to_string();
    let mut chars = message.chars();
    let message = chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    });
    IpcError::new(kind, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_map_to_core_options_and_validate() -> Result<(), IpcError> {
        let sql = ExportFormatDto::Sql {
            table: " users ".to_owned(),
            rows_per_statement: 50_000,
        };
        let Format::Insert(options) = sql.to_format(Dialect::MySql)? else {
            return Err(IpcError::invalid("not an insert"));
        };
        assert_eq!(options.table, "users");
        assert_eq!(options.rows_per_statement.get(), 10_000);
        assert_eq!(options.dialect, Dialect::MySql);
        let empty = ExportFormatDto::Sql {
            table: "  ".to_owned(),
            rows_per_statement: 10,
        };
        assert!(empty.to_format(Dialect::Postgres).is_err());
        let zero = ExportFormatDto::Sql {
            table: "t".to_owned(),
            rows_per_statement: 0,
        };
        assert!(zero.to_format(Dialect::Postgres).is_err());
        let lines = ExportFormatDto::Json { lines: true };
        assert_eq!(
            lines.to_format(Dialect::Sqlite)?,
            Format::Json(JsonLayout::Lines)
        );
        assert_eq!(lines.extension(), ("ndjson", "NDJSON"));
        Ok(())
    }

    #[test]
    fn events_serialise_with_camel_case_tags() -> Result<(), serde_json::Error> {
        let done = terminal(ExportOutcome::Written { rows: 3 }, "/tmp/x.csv".to_owned());
        let json = serde_json::to_value(done)?;
        assert_eq!(json["type"], "finished");
        assert_eq!(json["rows"], 3.0);
        let failed = terminal(
            ExportOutcome::Failed(ExportFailure::Write(ExportError::NoResult)),
            String::new(),
        );
        let json = serde_json::to_value(failed)?;
        assert_eq!(json["error"]["kind"], "invalid");
        assert_eq!(
            json["error"]["message"],
            "The statement returned no rows to export"
        );
        let request: ExportRequest = serde_json::from_value(serde_json::json!({
            "source": { "kind": "result", "sessionId": "s", "resultId": "r" },
            "format": { "kind": "csv", "delimiter": "tab", "header": true },
            "fileName": "result"
        }))?;
        assert_eq!(request.source.session_id(), "s");
        Ok(())
    }
}
