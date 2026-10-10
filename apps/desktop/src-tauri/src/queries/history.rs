// SPDX-License-Identifier: Apache-2.0
//! Records executed statements in the local store's history (docs/15 §2, FR-WS-01/02).
//!
//! A connection with "record query history" off records nothing: every
//! recorder here checks the profile's flag before it writes.
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use openrows_core::QueryEvent;
use openrows_core::script::ScriptEvent;
use openrows_store::{HistoryRecord, HistorySource, HistoryStatus, LocalStore};

/// The statement currently running.
struct Running {
    started_at_ms: i64,
    started: Instant,
    rows: Option<i64>,
}

impl Running {
    fn now() -> Self {
        Self {
            started_at_ms: now_ms(),
            started: Instant::now(),
            rows: None,
        }
    }
}

/// Turns one run's events into history records, one per executed statement.
pub(crate) struct HistoryLog {
    enabled: bool,
    profile_id: String,
    database: Option<String>,
    statements: Vec<String>,
    running: Option<Running>,
}

impl HistoryLog {
    pub(crate) fn new(
        enabled: bool,
        profile_id: String,
        database: Option<String>,
        statements: Vec<String>,
    ) -> Self {
        Self {
            enabled,
            profile_id,
            database,
            statements,
            running: None,
        }
    }

    /// Track `event`; returns a record when an executed statement finished.
    pub(crate) fn observe(&mut self, event: &ScriptEvent) -> Option<HistoryRecord> {
        if !self.enabled {
            return None;
        }
        let (status, error) = match &event.event {
            QueryEvent::Started { .. } => {
                self.running = Some(Running::now());
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
        let running = self.finished_run(status)?;
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

    /// The run that just ended. A statement that failed before it started (the
    /// session refused it) was still attempted, so it is recorded with no
    /// duration; one cancelled before it started never ran and is skipped.
    fn finished_run(&mut self, status: HistoryStatus) -> Option<Running> {
        match self.running.take() {
            Some(running) => Some(running),
            None if status == HistoryStatus::Error => Some(Running::now()),
            None => None,
        }
    }

    fn add_rows(&mut self, rows: u64) {
        if let Some(running) = self.running.as_mut() {
            let rows = i64::try_from(rows).unwrap_or(i64::MAX);
            running.rows = Some(running.rows.unwrap_or(0).saturating_add(rows));
        }
    }
}

/// One commit of a grid change set: every generated statement is recorded with
/// the batch's outcome and duration (the driver reports no per-statement counts).
pub(crate) struct CommitRecord<'a> {
    pub(crate) profile_id: &'a str,
    pub(crate) database: &'a str,
    pub(crate) started_at_ms: i64,
    pub(crate) duration: Duration,
    pub(crate) statements: &'a [String],
    /// The failure message when the batch did not commit.
    pub(crate) error: Option<&'a str>,
    /// Redaction policy and the driver dialect.
    pub(crate) privacy: (bool, openrows_sql::Dialect),
}

/// Append one history entry per statement of a commit batch (source `edit-commit`).
/// History is best effort: a failed write never fails the commit.
pub(crate) fn record_commit(local: &LocalStore, commit: &CommitRecord<'_>) {
    let duration_ms = i64::try_from(commit.duration.as_millis()).ok();
    for statement in commit.statements {
        super::history_privacy::record(
            local,
            HistoryRecord {
                profile_id: commit.profile_id.to_owned(),
                database: Some(commit.database.to_owned()),
                started_at_ms: commit.started_at_ms,
                duration_ms,
                status: if commit.error.is_some() {
                    HistoryStatus::Error
                } else {
                    HistoryStatus::Ok
                },
                rows: None,
                statement: statement.clone(),
                error_message: commit.error.map(str::to_owned),
                source: HistorySource::EditCommit,
            },
            commit.privacy,
        );
    }
}

/// Unix epoch milliseconds now, the `started_at` of a history record.
pub(crate) fn now_ms() -> i64 {
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

    fn at(statement: usize, event: QueryEvent) -> ScriptEvent {
        ScriptEvent { statement, event }
    }

    fn log(enabled: bool) -> HistoryLog {
        HistoryLog::new(
            enabled,
            "profile".to_owned(),
            Some("shop".to_owned()),
            vec![
                "SELECT 1".to_owned(),
                "UPDATE t SET a = 1 WHERE b".to_owned(),
            ],
        )
    }

    fn failure(message: &str) -> QueryEvent {
        QueryEvent::Failed {
            error: CoreError {
                kind: ErrorKind::Server,
                message: message.to_owned(),
                db_code: None,
            },
        }
    }

    #[test]
    fn completed_statements_record_rows_and_duration() {
        let mut log = log(true);
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
    fn failures_keep_the_message_and_cancelled_before_start_is_skipped() {
        let mut log = log(true);
        assert!(log.observe(&at(1, failure("boom"))).is_some());
        log.observe(&at(
            1,
            QueryEvent::Started {
                query: QueryId::new(),
            },
        ));
        log.observe(&at(1, QueryEvent::RowsAffected { count: 2 }));
        let record = log.observe(&at(1, failure("boom")));
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

    #[test]
    fn a_statement_refused_before_it_started_is_recorded_as_an_error() {
        let mut log = log(true);
        let record = log.observe(&at(0, failure("session is busy")));
        assert!(matches!(
            record,
            Some(HistoryRecord {
                status: HistoryStatus::Error,
                duration_ms: Some(_),
                rows: None,
                ref statement,
                ..
            }) if statement == "SELECT 1"
        ));
    }

    #[test]
    fn a_disabled_connection_records_nothing() {
        let mut log = log(false);
        let started = QueryEvent::Started {
            query: QueryId::new(),
        };
        assert!(log.observe(&at(0, started)).is_none());
        assert!(
            log.observe(&at(
                0,
                QueryEvent::Completed {
                    elapsed: Duration::ZERO
                }
            ))
            .is_none()
        );
        assert!(log.observe(&at(1, failure("boom"))).is_none());
    }

    #[test]
    fn a_commit_batch_records_each_statement_with_the_batch_outcome() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
        let local = LocalStore::open(&dir.path().join("openrows.sqlite"))
            .map_err(|error| error.to_string())?;
        let statements = vec![
            "UPDATE t SET a = $1 WHERE id = $2".to_owned(),
            "DELETE FROM t WHERE id = $1".to_owned(),
        ];
        record_commit(
            &local,
            &CommitRecord {
                profile_id: "p1",
                database: "shop",
                started_at_ms: 10,
                duration: Duration::from_millis(4),
                statements: &statements,
                error: Some("deadlock"),
                privacy: (false, openrows_sql::Dialect::Postgres),
            },
        );
        let entries = local
            .search_history(Some("p1"), "", 10)
            .map_err(|error| error.to_string())?;
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| {
            entry.record.source == HistorySource::EditCommit
                && entry.record.status == HistoryStatus::Error
                && entry.record.error_message.as_deref() == Some("deadlock")
        }));
        Ok(())
    }
}
