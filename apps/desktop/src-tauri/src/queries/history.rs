// SPDX-License-Identifier: Apache-2.0
//! Records every executed statement in the local store's history (docs/15 §2).
use openrows_core::QueryEvent;
use openrows_core::script::ScriptEvent;
use openrows_store::{HistoryRecord, HistorySource, HistoryStatus};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The statement currently running.
struct Running {
    started_at_ms: i64,
    started: Instant,
    rows: Option<i64>,
}

/// Turns one run's events into history records, one per started statement.
pub(crate) struct HistoryLog {
    profile_id: String,
    database: Option<String>,
    statements: Vec<String>,
    running: Option<Running>,
}

impl HistoryLog {
    pub(crate) fn new(
        profile_id: String,
        database: Option<String>,
        statements: Vec<String>,
    ) -> Self {
        Self {
            profile_id,
            database,
            statements,
            running: None,
        }
    }

    /// Track `event`; returns a record when a started statement finished.
    pub(crate) fn observe(&mut self, event: &ScriptEvent) -> Option<HistoryRecord> {
        let (status, error) = match &event.event {
            QueryEvent::Started { .. } => {
                self.running = Some(Running {
                    started_at_ms: now_ms(),
                    started: Instant::now(),
                    rows: None,
                });
                return None;
            }
            QueryEvent::ResultEnd { rows, .. } => {
                self.add_rows(*rows);
                return None;
            }
            QueryEvent::RowsAffected { count } => {
                self.add_rows(*count);
                return None;
            }
            QueryEvent::Completed { .. } => (HistoryStatus::Ok, None),
            QueryEvent::Failed { error } => (HistoryStatus::Error, Some(error.message.clone())),
            QueryEvent::Cancelled => (HistoryStatus::Cancelled, None),
            _ => return None,
        };
        let running = self.running.take()?;
        Some(HistoryRecord {
            profile_id: self.profile_id.clone(),
            database: self.database.clone(),
            started_at_ms: running.started_at_ms,
            duration_ms: i64::try_from(running.started.elapsed().as_millis()).ok(),
            status,
            rows: running.rows,
            statement: self
                .statements
                .get(event.statement)
                .cloned()
                .unwrap_or_default(),
            error_message: error,
            source: HistorySource::Editor,
        })
    }

    fn add_rows(&mut self, rows: u64) {
        if let Some(running) = self.running.as_mut() {
            let rows = i64::try_from(rows).unwrap_or(i64::MAX);
            running.rows = Some(running.rows.unwrap_or(0).saturating_add(rows));
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_core::{CoreError, ErrorKind};
    use openrows_types::{QueryId, ResultId};
    use std::time::Duration;

    fn at(statement: usize, event: QueryEvent) -> ScriptEvent {
        ScriptEvent { statement, event }
    }

    fn log() -> HistoryLog {
        HistoryLog::new(
            "profile".to_owned(),
            Some("shop".to_owned()),
            vec![
                "SELECT 1".to_owned(),
                "UPDATE t SET a = 1 WHERE b".to_owned(),
            ],
        )
    }

    #[test]
    fn completed_statements_record_rows_and_duration() {
        let mut log = log();
        let started = QueryEvent::Started {
            query: QueryId::new(),
        };
        assert!(log.observe(&at(0, started)).is_none());
        let end = QueryEvent::ResultEnd {
            result: ResultId::new(),
            rows: 7,
            truncated: false,
        };
        assert!(log.observe(&at(0, end)).is_none());
        let done = QueryEvent::Completed {
            elapsed: Duration::from_millis(3),
        };
        let record = log.observe(&at(0, done)).ok_or("no record");
        assert!(matches!(
            record,
            Ok(HistoryRecord {
                status: HistoryStatus::Ok,
                rows: Some(7),
                ref statement,
                ref database,
                source: HistorySource::Editor,
                ..
            }) if statement == "SELECT 1" && database.as_deref() == Some("shop")
        ));
    }

    #[test]
    fn failures_keep_the_message_and_unstarted_statements_are_skipped() {
        let mut log = log();
        let failed = QueryEvent::Failed {
            error: CoreError {
                kind: ErrorKind::Server,
                message: "boom".to_owned(),
                db_code: None,
            },
        };
        assert!(log.observe(&at(1, failed.clone())).is_none());
        log.observe(&at(
            1,
            QueryEvent::Started {
                query: QueryId::new(),
            },
        ));
        log.observe(&at(1, QueryEvent::RowsAffected { count: 2 }));
        let record = log.observe(&at(1, failed));
        assert!(matches!(
            record,
            Some(HistoryRecord {
                status: HistoryStatus::Error,
                rows: Some(2),
                error_message: Some(ref message),
                ..
            }) if message == "boom"
        ));
        assert!(log.observe(&at(1, QueryEvent::Cancelled)).is_none());
    }
}
