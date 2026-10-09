// SPDX-License-Identifier: Apache-2.0
//! Streamed export (FR-IO-01, docs/06 §8): rows go from the driver, or from
//! a complete stored result, to an [`Exporter`] on a blocking thread and
//! never through the webview.
//!
//! A statement export runs on the session actor like any query, but its
//! batches go to an [`ExportSink`] instead of the result store. The sink
//! forwards them over a bounded channel to [`openrows_export::drain`]; when
//! the disk is slower than the server, the full channel blocks the actor's
//! push, which is back-pressure on the server stream instead of unbounded
//! memory. Cancel uses the query's normal out-of-band path; the writer then
//! sees the channel close without `Finish` and deletes its temporary file.
use crate::error::ErrorKind;
use crate::script::{ScriptCanceller, ScriptOutcome, follow};
use crate::sink::{PROGRESS_INTERVAL, Throttle};
use crate::{BatchSink, CoreError, QueryEvent, SessionManager};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::SinkFlow;
use openrows_export::{ExportError, Exporter, Message, drain, write_all};
use openrows_sql::select::BoundSql;
use openrows_types::{ResultId, SessionId};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

/// Batches buffered between the session actor and the file writer.
const PIPE_CAPACITY: usize = 8;

/// Reports rows written so far; called from the writer thread.
pub type Progress = Box<dyn FnMut(u64) + Send>;

/// One export: where it writes, how it is cancelled and who hears progress.
pub struct ExportJob {
    /// Encoder and temporary file for the chosen format and path.
    pub exporter: Exporter,
    /// Cancels the statement (or stops reading stored batches).
    pub cancel: ScriptCanceller,
    /// Rows written, at most 10 calls per second.
    pub progress: Progress,
}

/// How an export ended; only `Written` leaves a file behind.
#[derive(Debug)]
pub enum ExportOutcome {
    /// The file is complete and in place.
    Written {
        /// Rows written.
        rows: u64,
    },
    /// The user cancelled; the temporary file was deleted.
    Cancelled,
    /// The statement or the file failed; the temporary file was deleted.
    Failed(ExportFailure),
}

/// Why an export failed.
#[derive(Debug)]
pub enum ExportFailure {
    /// The statement failed on the server or the session broke.
    Query(CoreError),
    /// Encoding or writing the file failed.
    Write(ExportError),
}

/// Run `statement` on `session` and stream its rows into the job's file.
#[tracing::instrument(name = "export", skip_all, fields(source = "query"))]
pub async fn export_statement(
    manager: &SessionManager,
    session: SessionId,
    statement: BoundSql,
    job: ExportJob,
) -> ExportOutcome {
    let outcome = stream_statement(manager, session, statement, job).await;
    log_outcome(&outcome);
    outcome
}

async fn stream_statement(
    manager: &SessionManager,
    session: SessionId,
    statement: BoundSql,
    job: ExportJob,
) -> ExportOutcome {
    let ExportJob {
        exporter,
        cancel,
        progress,
    } = job;
    let (sender, receiver) = sync_channel(PIPE_CAPACITY);
    let progress = throttled(progress);
    let writer = tokio::task::spawn_blocking(move || drain(exporter, receiver, progress));
    let sink = Arc::new(ExportSink::new(sender));
    let mut failure = None;
    let outcome = if cancel.is_cancelled() {
        ScriptOutcome::Cancelled
    } else {
        let submitted = manager.execute_into(session, statement, sink.clone());
        follow(submitted, &cancel, &mut |event| {
            if let QueryEvent::Failed { error } = event {
                failure = Some(error);
            }
        })
        .await
    };
    sink.close(outcome == ScriptOutcome::Completed);
    let written = writer.await.unwrap_or(Err(ExportError::Aborted));
    match outcome {
        ScriptOutcome::Completed => conclude(written),
        ScriptOutcome::Cancelled => ExportOutcome::Cancelled,
        ScriptOutcome::Failed => {
            ExportOutcome::Failed(ExportFailure::Query(failure.unwrap_or_else(|| {
                CoreError::new(ErrorKind::Internal, "the export query failed")
            })))
        }
    }
}

/// Write a complete stored result (`schema` and a lazy, fallible reader of
/// its batches, such as [`openrows_results::StoredBatches`]) into the job's file.
#[tracing::instrument(name = "export", skip_all, fields(source = "result"))]
pub async fn export_batches<I, E>(
    (schema, batches): (SchemaRef, I),
    job: ExportJob,
) -> ExportOutcome
where
    I: IntoIterator<Item = Result<RecordBatch, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let outcome = write_batches((schema, batches), job).await;
    log_outcome(&outcome);
    outcome
}

/// Log how an export ended: its kind and row count, never the failure text,
/// because driver and encoder messages can echo row values.
fn log_outcome(outcome: &ExportOutcome) {
    match outcome {
        ExportOutcome::Written { rows } => {
            tracing::info!(outcome = "written", rows, "export finished");
        }
        ExportOutcome::Cancelled => tracing::info!(outcome = "cancelled", "export finished"),
        ExportOutcome::Failed(ExportFailure::Query(error)) => {
            tracing::warn!(outcome = "failed", stage = "query", kind = ?error.kind, "export finished");
        }
        ExportOutcome::Failed(ExportFailure::Write(_)) => {
            tracing::warn!(outcome = "failed", stage = "write", "export finished");
        }
    }
}

async fn write_batches<I, E>((schema, batches): (SchemaRef, I), job: ExportJob) -> ExportOutcome
where
    I: IntoIterator<Item = Result<RecordBatch, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let ExportJob {
        exporter,
        cancel,
        progress,
    } = job;
    let stop = cancel.clone();
    let progress = throttled(progress);
    let written = tokio::task::spawn_blocking(move || {
        write_all(
            exporter,
            (schema, batches),
            || stop.is_cancelled(),
            progress,
        )
    })
    .await
    .unwrap_or(Err(ExportError::Aborted));
    if cancel.is_cancelled() {
        return ExportOutcome::Cancelled;
    }
    conclude(written)
}

fn conclude(written: Result<u64, ExportError>) -> ExportOutcome {
    match written {
        Ok(rows) => ExportOutcome::Written { rows },
        Err(error) => ExportOutcome::Failed(ExportFailure::Write(error)),
    }
}

/// At most one progress report per [`PROGRESS_INTERVAL`] (≤ 10 Hz, docs/06 §4).
fn throttled(mut progress: Progress) -> impl FnMut(u64) + Send {
    let mut throttle = Throttle::new(PROGRESS_INTERVAL);
    move |rows| {
        if throttle.ready(Instant::now()) {
            progress(rows);
        }
    }
}

/// The per-statement [`BatchSink`]: forwards every result set to the writer.
///
/// A statement yields one result set, except when the driver continues it
/// in a new one after a late text fallback; the exporter appends those and
/// refuses results with other columns.
struct ExportSink {
    sender: Mutex<Option<SyncSender<Message>>>,
}

impl ExportSink {
    fn new(sender: SyncSender<Message>) -> Self {
        Self {
            sender: Mutex::new(Some(sender)),
        }
    }

    /// False once the writer is gone (it failed) or the sink was closed.
    fn send(&self, message: Message) -> bool {
        self.sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|sender| sender.send(message).is_ok())
    }

    /// Stop forwarding; `finish` tells the writer every row arrived.
    fn close(&self, finish: bool) {
        if finish {
            self.send(Message::Finish);
        }
        self.sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
    }
}

impl BatchSink for ExportSink {
    fn begin(&self, _result: ResultId, schema: SchemaRef) {
        // A failed send means the writer already stopped; `push` reports it.
        self.send(Message::Begin(schema));
    }

    /// `Stop` once the writer failed, which ends the server stream.
    fn push(&self, _result: ResultId, batch: RecordBatch) -> SinkFlow {
        if self.send(Message::Batch(batch)) {
            SinkFlow::Continue
        } else {
            SinkFlow::Stop
        }
    }

    fn end(&self, _result: ResultId) {}
}
