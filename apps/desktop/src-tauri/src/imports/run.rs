// SPDX-License-Identifier: Apache-2.0
//! A started dry run or import: feed the file, load it on the dedicated
//! connection, throttle progress to the webview and send the terminal event.
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use openrows_driver_api::{Connection, DriverError};
use openrows_import::{
    CancellationToken, Control, Failure, FeedSource, LoadPlan, Outcome, RunMode, Summary, load,
    spawn_feed,
};
use openrows_sql::StatementKind;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::Imports;
use super::commands::file_error;
use super::dto::{ImportEventDto, ImportSummaryDto, count};
use crate::connections::catalog_cache::after_statements;
use crate::connections::{IpcError, IpcErrorKind};
use crate::tables::TableTarget;

/// Least time between progress events.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Everything a started job needs after `import_start` returned.
pub(crate) struct Job {
    pub(crate) import_id: String,
    pub(crate) connection: Box<dyn Connection>,
    pub(crate) plan: LoadPlan,
    pub(crate) feed: FeedSource,
    /// File size in bytes, for the share read.
    pub(crate) size: u64,
    pub(crate) cancel: CancellationToken,
    pub(crate) events: Channel<ImportEventDto>,
    pub(crate) target: TableTarget,
}

pub(crate) async fn execute(app: AppHandle, job: Job) {
    let Job {
        import_id,
        mut connection,
        plan,
        feed,
        size,
        cancel,
        events,
        target,
    } = job;
    let read = Arc::new(AtomicU64::new(0));
    let receiver = spawn_feed(feed, Arc::clone(&read));
    let control = Control {
        cancel,
        progress: progress(events.clone(), read, size),
    };
    let outcome = load(connection.as_mut(), &plan, receiver, control).await;
    // Closing rolls back anything still open; the outcome is already known.
    let _ = connection.close().await;
    if plan.create.is_some() && plan.mode == RunMode::Import {
        after_statements(
            &app,
            (&target.profile_id, &target.database),
            [StatementKind::Ddl],
        );
    }
    log(&outcome, plan.mode);
    // A closed channel means the window is gone; the job already ended.
    let _ = events.send(terminal(&outcome));
    app.state::<Imports>().forget(&import_id);
}

fn progress(
    events: Channel<ImportEventDto>,
    read: Arc<AtomicU64>,
    size: u64,
) -> Box<dyn FnMut(&Summary) + Send> {
    let started = Instant::now();
    let mut last: Option<Instant> = None;
    Box::new(move |summary| {
        if last.is_some_and(|last| last.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        last = Some(Instant::now());
        let seconds = started.elapsed().as_secs_f64().max(0.001);
        let fraction = if size == 0 {
            0.0
        } else {
            (count(read.load(Ordering::Relaxed)) / count(size)).min(1.0)
        };
        // A closed channel means the window is gone; the job still finishes.
        let _ = events.send(ImportEventDto::Progress {
            rows: count(summary.rows),
            inserted: count(summary.inserted),
            failed: count(summary.failed),
            fraction,
            rows_per_second: (count(summary.rows) / seconds).round(),
        });
    })
}

/// Log how a job ended: counts and kind only, never values or messages.
fn log(outcome: &Outcome, mode: RunMode) {
    match outcome {
        Outcome::Finished(summary) => tracing::info!(
            ?mode,
            rows = summary.rows,
            inserted = summary.inserted,
            failed = summary.failed,
            "import finished"
        ),
        Outcome::Cancelled(_) => tracing::info!(?mode, "import cancelled"),
        Outcome::Failed(_, summary) => tracing::warn!(
            ?mode,
            inserted = summary.inserted,
            failed = summary.failed,
            "import failed"
        ),
    }
}

pub(crate) fn terminal(outcome: &Outcome) -> ImportEventDto {
    match outcome {
        Outcome::Finished(summary) => ImportEventDto::Finished {
            summary: ImportSummaryDto::from(summary),
        },
        Outcome::Cancelled(summary) => ImportEventDto::Cancelled {
            summary: ImportSummaryDto::from(summary),
        },
        Outcome::Failed(failure, summary) => ImportEventDto::Failed {
            error: failure_error(failure, summary),
            summary: ImportSummaryDto::from(summary),
        },
    }
}

fn failure_error(failure: &Failure, summary: &Summary) -> IpcError {
    match failure {
        Failure::Batch { error, lines } => {
            let mut ipc = IpcError::from(error);
            if let Some((first, last)) = lines {
                ipc.message = format!(
                    "Rows on lines {first}–{last} were rejected: {}",
                    ipc.message
                );
            }
            ipc
        }
        Failure::Setup(error) | Failure::Commit(error @ DriverError::Server { .. }) => {
            IpcError::from(error)
        }
        Failure::Commit(error) => IpcError::new(
            IpcErrorKind::Network,
            format!(
                "The commit could not be confirmed ({}); check the table before importing again",
                IpcError::from(error).message
            ),
        ),
        Failure::File(error) => file_error(error),
        Failure::BadRow => IpcError::invalid(match summary.errors.last() {
            Some(error) => format!("Stopped at a bad row ({error})"),
            None => "Stopped at a bad row".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_import::RowError;

    #[test]
    fn failures_name_lines_and_bad_rows() -> Result<(), serde_json::Error> {
        let summary = Summary {
            rows: 3,
            inserted: 2,
            errors: vec![RowError::row(4, "expected 2 fields but found 1")],
            ..Summary::default()
        };
        let batch = Failure::Batch {
            error: DriverError::Server {
                code: "23505".to_owned(),
                message: "duplicate key".to_owned(),
            },
            lines: Some((2, 9)),
        };
        let event = terminal(&Outcome::Failed(batch, summary.clone()));
        let json = serde_json::to_value(event)?;
        assert_eq!(json["type"], "failed");
        assert_eq!(json["error"]["code"], "23505");
        assert_eq!(
            json["error"]["message"],
            "Rows on lines 2–9 were rejected: duplicate key"
        );
        assert_eq!(json["summary"]["inserted"], 2.0);
        assert_eq!(json["summary"]["errors"][0]["line"], 4.0);
        let bad = failure_error(&Failure::BadRow, &summary);
        assert_eq!(
            bad.message,
            "Stopped at a bad row (line 4: expected 2 fields but found 1)"
        );
        let finished = serde_json::to_value(terminal(&Outcome::Finished(Summary::default())))?;
        assert_eq!(finished["summary"]["conflicts"], serde_json::Value::Null);
        Ok(())
    }
}
