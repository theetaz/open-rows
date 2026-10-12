// SPDX-License-Identifier: Apache-2.0
//! Own-session diagnostics for `SQLite` and libSQL files (ADR-0011): the
//! statements this app itself ran against this file, plus cheap file/page
//! PRAGMAs and file sizes. Integrity and quick checks never run per tick.
mod file;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use openrows_driver_api::{DriverError, ExecSummary, FileSource};
use openrows_monitor::{
    Descriptor, Execution, Ledger, MonitorReader, ReadFuture, Reading, SampleState, Scope, Source,
};

use crate::session::SqliteSession;
use crate::stream::Job;

/// The ADR-0011 boundary shown beside every embedded panel.
pub(crate) const OWN: &str = "Own OpenRows sessions only — other applications are not visible.";

/// The file a session belongs to and the ledger its executions feed.
pub(crate) struct Target {
    pub(crate) ledger: Arc<Ledger>,
    /// Main database file; in-memory databases have none.
    pub(crate) path: Option<PathBuf>,
}

impl Target {
    pub(crate) fn of(source: &FileSource) -> Self {
        let (key, path) = match source {
            FileSource::Disk { path, .. } => {
                let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                (format!("sqlite:{}", path.display()), Some(path))
            }
            FileSource::Memory { name } => (format!("sqlite-memory:{name}"), None),
        };
        Self {
            ledger: Ledger::shared(&key),
            path,
        }
    }
}

/// Record one finished own statement with its wall time and step counters.
pub(crate) fn record(
    ledger: &Ledger,
    job: &Job,
    started: Instant,
    result: &Result<ExecSummary, DriverError>,
) {
    ledger.record(
        &job.sql,
        Execution {
            elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
            rows: result.as_ref().map_or(0, |summary| summary.rows),
            failed: result.is_err(),
            steps: job.steps,
        },
    );
}

impl MonitorReader for SqliteSession {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        Box::pin(discover(self))
    }

    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading> {
        Box::pin(async move {
            match source {
                Source::Statements => self.monitor.ledger.statements(),
                Source::Database => file::read(self, deadline_ms).await,
                other => Reading::missing(unavailable(other)),
            }
        })
    }

    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancel = openrows_driver_api::Session::cancel_handle(self);
        Box::pin(async move {
            if let Some(cancel) = cancel {
                let _ = cancel.cancel().await;
            }
        })
    }
}

async fn discover(session: &mut SqliteSession) -> Vec<Descriptor> {
    let probe = file::read(session, 1_000).await;
    let journal = probe
        .rows
        .first()
        .and_then(|row| row.last().cloned().flatten())
        .unwrap_or_else(|| "unknown".into());
    Source::ALL
        .into_iter()
        .map(|source| {
            let state = match source {
                Source::Database => probe.state.clone(),
                Source::Statements => SampleState::Ok,
                other => unavailable(other),
            };
            descriptor(source, state, &journal)
        })
        .collect()
}

fn descriptor(source: Source, state: SampleState, journal: &str) -> Descriptor {
    let coverage = match source {
        Source::Database => format!(
            "Cheap PRAGMAs only (page_size, page_count, freelist_count, journal_mode = {journal}), file/WAL sizes and the WAL index checkpoint position, plus own request totals. integrity_check and quick_check never run on a tick."
        ),
        Source::Statements => format!(
            "{OWN} Client wall time, rows, errors and sqlite3_stmt_status FULLSCAN_STEP/SORT/VM_STEP per statement shape since this app first opened the file."
        ),
        _ => OWN.to_owned(),
    };
    Descriptor {
        source,
        state,
        scope: Scope::Database,
        interval_ms: source.floor_ms(5_000),
        guidance: None,
        coverage,
    }
}

fn unavailable(source: Source) -> SampleState {
    let reason = match source {
        Source::Activity => "SQLite has no server process list or lock view",
        Source::Replication => "SQLite files have no replication",
        Source::Tables | Source::Indexes => {
            "Per-table/index statistics need a full dbstat scan, which is not run on a tick"
        }
        Source::Io => "SQLite exposes no file I/O or wait counters to other connections",
        Source::Database | Source::Statements => "Unavailable",
    };
    SampleState::Unavailable(format!("{reason}. {OWN}"))
}

/// The monitor connection waits briefly for a writer, never the user default.
pub(crate) const MONITOR_BUSY: Duration = Duration::from_millis(100);
