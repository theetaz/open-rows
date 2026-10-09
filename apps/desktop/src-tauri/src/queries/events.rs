// SPDX-License-Identifier: Apache-2.0
//! The query event stream sent over the `query_execute` channel (docs/06 §4).
use crate::connections::IpcError;
use arrow_schema::SchemaRef;
use openrows_core::QueryEvent;
use openrows_core::script::{ScriptEvent, ScriptOutcome};
use openrows_driver_api::NoticeLevel;
use openrows_driver_api::metadata::SOURCE_TYPE;
use openrows_results::ResultStore;
use openrows_types::ResultId;
use serde::Serialize;
use specta::Type;
use std::time::Duration;

/// A result column as the grid header shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ColumnDto {
    name: String,
    /// Database type name, e.g. `int4` or `timestamptz`.
    source_type: Option<String>,
}

/// Server notice severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NoticeLevelDto {
    Info,
    Warning,
}

/// How a whole run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RunOutcomeDto {
    Completed,
    Failed,
    Cancelled,
}

/// Progress of one run; `statement` indexes the reply's statements.
/// `finished` is always the last event.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum QueryEventDto {
    StatementStarted {
        statement: u32,
    },
    ResultBegin {
        statement: u32,
        result_id: String,
        columns: Vec<ColumnDto>,
    },
    /// Rows stored so far; at most 10 per second.
    Progress {
        statement: u32,
        result_id: String,
        rows: u32,
    },
    /// Final row count; `truncated` when the row cap or memory budget stopped it.
    ResultEnd {
        statement: u32,
        result_id: String,
        rows: u32,
        truncated: bool,
    },
    RowsAffected {
        statement: u32,
        count: u32,
    },
    Notice {
        statement: u32,
        level: NoticeLevelDto,
        message: String,
    },
    StatementDone {
        statement: u32,
        elapsed_ms: u32,
    },
    Failed {
        statement: u32,
        error: IpcError,
    },
    Cancelled {
        statement: u32,
    },
    Finished {
        outcome: RunOutcomeDto,
        elapsed_ms: u32,
    },
}

/// The run's terminal event.
pub(crate) fn finished(outcome: ScriptOutcome, elapsed: Duration) -> QueryEventDto {
    QueryEventDto::Finished {
        outcome: match outcome {
            ScriptOutcome::Completed => RunOutcomeDto::Completed,
            ScriptOutcome::Failed => RunOutcomeDto::Failed,
            ScriptOutcome::Cancelled => RunOutcomeDto::Cancelled,
        },
        elapsed_ms: millis(elapsed),
    }
}

/// Counts above `u32::MAX` (4.29 billion) saturate; result rows are capped far lower.
fn saturate(count: u64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn millis(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

/// Translate a core event; row counts come from the store, which may have cut
/// the last batch at the row cap.
pub(crate) fn translate(event: ScriptEvent, store: &ResultStore) -> QueryEventDto {
    let statement = saturate(event.statement as u64);
    let stored = |result: ResultId, rows: u64| {
        store
            .rows(&result.to_string())
            .map_or((saturate(rows), false), |(rows, cut)| {
                (saturate(rows as u64), cut)
            })
    };
    match event.event {
        QueryEvent::Started { .. } => QueryEventDto::StatementStarted { statement },
        QueryEvent::ResultBegin { result, schema } => QueryEventDto::ResultBegin {
            statement,
            result_id: result.to_string(),
            columns: columns(&schema),
        },
        QueryEvent::Progress { result, rows, .. } => QueryEventDto::Progress {
            statement,
            result_id: result.to_string(),
            rows: stored(result, rows).0,
        },
        QueryEvent::ResultEnd {
            result,
            rows,
            truncated,
        } => {
            let (rows, cut) = stored(result, rows);
            QueryEventDto::ResultEnd {
                statement,
                result_id: result.to_string(),
                rows,
                truncated: truncated || cut,
            }
        }
        other => terminal_or_message(statement, other),
    }
}

fn terminal_or_message(statement: u32, event: QueryEvent) -> QueryEventDto {
    match event {
        QueryEvent::RowsAffected { count } => QueryEventDto::RowsAffected {
            statement,
            count: saturate(count),
        },
        QueryEvent::Notice { level, message } => QueryEventDto::Notice {
            statement,
            level: match level {
                NoticeLevel::Info => NoticeLevelDto::Info,
                NoticeLevel::Warning => NoticeLevelDto::Warning,
            },
            message,
        },
        QueryEvent::Completed { elapsed } => QueryEventDto::StatementDone {
            statement,
            elapsed_ms: millis(elapsed),
        },
        QueryEvent::Failed { error } => QueryEventDto::Failed {
            statement,
            error: error.into(),
        },
        _ => QueryEventDto::Cancelled { statement },
    }
}

fn columns(schema: &SchemaRef) -> Vec<ColumnDto> {
    schema
        .fields()
        .iter()
        .map(|field| ColumnDto {
            name: field.name().clone(),
            source_type: field.metadata().get(SOURCE_TYPE).cloned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int32Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use openrows_results::{Append, Limits};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn event(event: QueryEvent) -> ScriptEvent {
        ScriptEvent {
            statement: 1,
            event,
        }
    }

    #[test]
    fn result_end_reports_the_rows_the_store_kept() -> Result<(), Box<dyn std::error::Error>> {
        let field = Field::new("n", DataType::Int32, true)
            .with_metadata(HashMap::from([(SOURCE_TYPE.to_owned(), "int4".to_owned())]));
        let schema = Arc::new(Schema::new(vec![field]));
        let store = ResultStore::with_limits(Limits {
            max_rows: 2,
            ..Limits::default()
        });
        let result = ResultId::new();
        store.begin(&result.to_string(), schema.clone());
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3]))],
        )?;
        assert_eq!(store.append(&result.to_string(), batch), Append::Full);
        let begin = translate(event(QueryEvent::ResultBegin { result, schema }), &store);
        let json = serde_json::to_value(&begin)?;
        assert_eq!(json["type"], "resultBegin");
        assert_eq!(json["columns"][0]["sourceType"], "int4");
        let end = translate(
            event(QueryEvent::ResultEnd {
                result,
                rows: 3,
                truncated: false,
            }),
            &store,
        );
        assert_eq!(
            end,
            QueryEventDto::ResultEnd {
                statement: 1,
                result_id: result.to_string(),
                rows: 2,
                truncated: true,
            }
        );
        Ok(())
    }

    #[test]
    fn terminal_events_keep_the_statement_and_sqlstate() -> Result<(), serde_json::Error> {
        let store = ResultStore::default();
        let failed = translate(
            event(QueryEvent::Failed {
                error: openrows_core::CoreError {
                    kind: openrows_core::ErrorKind::Server,
                    message: "relation \"nope\" does not exist".to_owned(),
                    db_code: Some("42P01".to_owned()),
                },
            }),
            &store,
        );
        let json = serde_json::to_value(failed)?;
        assert_eq!(json["type"], "failed");
        assert_eq!(json["statement"], 1);
        assert_eq!(json["error"]["code"], "42P01");
        let done = finished(ScriptOutcome::Cancelled, Duration::from_millis(5));
        assert_eq!(serde_json::to_value(done)?["outcome"], "cancelled");
        Ok(())
    }
}
