// SPDX-License-Identifier: Apache-2.0
//! Request, reply and event types of `export_start`.
use super::format_dto::ExportFormatDto;
use crate::connections::{IpcError, IpcErrorKind};
use crate::tables::TableTarget;
use openrows_core::export::{ExportFailure, ExportOutcome};
use openrows_export::ExportError;
use serde::{Deserialize, Serialize};
use specta::Type;

/// A schema of a saved connection's database.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaTarget {
    pub(crate) profile_id: String,
    pub(crate) database: String,
    pub(crate) schema: String,
}

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
    /// Some rows of a stored result, by their 0-based position in it.
    Rows {
        session_id: String,
        result_id: String,
        rows: Vec<u32>,
    },
    /// Run one read-only statement again and stream all of its rows (a
    /// result the row limit cut short).
    Query { session_id: String, sql: String },
    /// Every row of a table or view.
    Table {
        session_id: String,
        target: TableTarget,
    },
    /// Every row of several tables of one schema: one file per table in a
    /// folder, or (Excel) one workbook with a sheet per table.
    Tables {
        session_id: String,
        target: SchemaTarget,
        tables: Vec<String>,
        workbook: bool,
    },
}

impl ExportSourceDto {
    pub(crate) fn session_id(&self) -> &str {
        match self {
            Self::Result { session_id, .. }
            | Self::Rows { session_id, .. }
            | Self::Query { session_id, .. }
            | Self::Table { session_id, .. }
            | Self::Tables { session_id, .. } => session_id,
        }
    }
}

/// Start an export; the native save panel (or a folder panel for one file
/// per table) picks the destination.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportRequest {
    pub(crate) source: ExportSourceDto,
    pub(crate) format: ExportFormatDto,
    /// Suggested file name without extension, e.g. the table name.
    pub(crate) file_name: String,
}

/// `started` once the destination was chosen; `dismissed` when the panel was cancelled.
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
        /// Name of the file, or of the folder for one file per table.
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
    /// Rows written so far; at most 10 per second. `detail` names the
    /// table being written when there are several.
    Progress {
        rows: f64,
        detail: Option<String>,
    },
    /// The file (or folder of files) is complete at `path`.
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
        ExportError::Io(_) | ExportError::InvalidPath | ExportError::Read(_) => {
            IpcErrorKind::Storage
        }
        ExportError::UnsupportedType { .. } => IpcErrorKind::Unsupported,
        ExportError::Aborted => IpcErrorKind::Cancelled,
        ExportError::InvalidName(_)
        | ExportError::NoColumns
        | ExportError::NoResult
        | ExportError::SchemaMismatch
        | ExportError::Unencodable(_)
        | ExportError::Excel(_)
        | ExportError::Parquet(_)
        | ExportError::OneTablePerFile => IpcErrorKind::Invalid,
    };
    IpcError::new(kind, capitalised(&error.to_string()))
}

pub(crate) fn capitalised(message: &str) -> String {
    let mut chars = message.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "source": {
                "kind": "tables",
                "sessionId": "s",
                "target": { "profileId": "p", "database": "d", "schema": "public" },
                "tables": ["a", "b"],
                "workbook": true
            },
            "format": { "kind": "xlsx", "header": true },
            "fileName": "public"
        }))?;
        assert_eq!(request.source.session_id(), "s");
        let rows: ExportSourceDto = serde_json::from_value(serde_json::json!({
            "kind": "rows", "sessionId": "s", "resultId": "r", "rows": [3, 1]
        }))?;
        assert!(matches!(rows, ExportSourceDto::Rows { rows, .. } if rows == [3, 1]));
        Ok(())
    }
}
