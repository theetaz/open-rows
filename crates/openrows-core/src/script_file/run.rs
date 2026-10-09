// SPDX-License-Identifier: Apache-2.0
//! Running a script file on a session (FR-IO-03): a reader thread splits
//! the file into batches of statements, and each batch runs through
//! [`run_script`] with its Return-on-Error semantics. Result rows are
//! discarded; only counts, the current statement and errors are reported.
use super::reader::{ScriptFileError, ScriptReader};
use super::scan::{FileStamp, preview};
use super::tally::{FileEvent, Tally};
use crate::safety::{SafetyPolicy, Verdict, verdict};
use crate::script::{Script, ScriptCanceller, ScriptOutcome, run_bound, run_script};
use crate::{BatchSink, QueryEvent, SessionManager};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::SinkFlow;
use openrows_sql::select::BoundSql;
use openrows_sql::{Dialect, ScriptStatement, classify};
use openrows_types::{ResultId, SessionId};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Statements handed to the script runner at once.
const BATCH: usize = 64;
/// Batches read ahead of the session.
const READ_AHEAD: usize = 4;

/// What to run and how.
#[derive(Clone, Copy, Debug)]
pub struct FileRun<'a> {
    /// The script file.
    pub path: &'a Path,
    /// The connection's safety switches and dialect.
    pub policy: SafetyPolicy,
    /// End at the first failed statement; otherwise run on and collect errors.
    pub stop_on_error: bool,
    /// Run everything in one transaction, rolled back on error or cancel
    /// (engines with transactional DDL only; errors always stop it).
    pub transaction: bool,
    /// The file as it was checked; a changed file is refused.
    pub stamp: Option<FileStamp>,
}

/// How a file run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileOutcome {
    /// Every statement ran without error.
    Completed,
    /// Statements failed, or the run stopped early (see the summary message).
    Failed,
    /// The user cancelled.
    Cancelled,
}

/// The end of a file run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSummary {
    /// How it ended.
    pub outcome: FileOutcome,
    /// Statements that ran (succeeded or failed).
    pub statements: u64,
    /// Statements that failed.
    pub errors: u64,
    /// Rows the statements reported as inserted, updated or deleted.
    pub rows_affected: u64,
    /// The transaction was rolled back.
    pub rolled_back: bool,
    /// Why the run stopped before the end of the file, when not a statement error.
    pub message: Option<String>,
}

/// Whether `dialect`'s engine can roll back DDL, which a whole-file
/// transaction needs (`MySQL` commits DDL implicitly).
#[must_use]
pub fn supports_transaction(dialect: Dialect) -> bool {
    matches!(dialect, Dialect::Postgres | Dialect::Sqlite)
}

/// How the statements ended, before the transaction is settled.
#[derive(Debug, PartialEq)]
enum Ended {
    Completed,
    Failed,
    Cancelled,
    Stopped(String),
}

/// One read step: a batch of statements or the read failure.
enum Read {
    Batch(Vec<ScriptStatement>),
    Failed(ScriptFileError),
}

/// Result rows of a file run are not kept.
struct Discard;

impl BatchSink for Discard {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}

    fn push(&self, _result: ResultId, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }

    fn end(&self, _result: ResultId) {}
}

/// Run the statements of `run.path` on `session` in order, reporting
/// progress (at most 10 per second) and each error through `emit`.
#[tracing::instrument(name = "script.file", skip_all, fields(transaction = run.transaction))]
pub async fn run_file(
    manager: &SessionManager,
    session: SessionId,
    run: FileRun<'_>,
    cancel: &ScriptCanceller,
    emit: impl FnMut(FileEvent),
) -> FileSummary {
    let mut tally = Tally::new(emit);
    let (ended, rolled_back) = match refusal(&run) {
        Some(reason) => (Ended::Stopped(reason), false),
        None => transacted(manager, session, run, cancel, &mut tally).await,
    };
    let (outcome, message) = match ended {
        Ended::Completed => (FileOutcome::Completed, None),
        Ended::Failed => (FileOutcome::Failed, None),
        Ended::Cancelled => (FileOutcome::Cancelled, None),
        Ended::Stopped(message) => (FileOutcome::Failed, Some(message)),
    };
    tracing::info!(?outcome, statements = tally.ran(), "script file finished");
    tally.summary(outcome, rolled_back, message)
}

/// Why the run may not start: a changed file or an unsupported transaction.
fn refusal(run: &FileRun<'_>) -> Option<String> {
    if run.transaction && !supports_transaction(run.policy.dialect) {
        return Some(
            "This database commits DDL implicitly, so a file cannot run in one transaction"
                .to_owned(),
        );
    }
    let expected = run.stamp?;
    match FileStamp::of(run.path) {
        Ok(stamp) if stamp == expected => None,
        Ok(_) => Some("The file changed after it was checked; check it again".to_owned()),
        Err(error) => Some(capitalised(&error.to_string())),
    }
}

/// The statements, inside `BEGIN … COMMIT` when asked; anything but
/// success rolls back. Returns how it ended and whether it rolled back.
async fn transacted<E: FnMut(FileEvent)>(
    manager: &SessionManager,
    session: SessionId,
    run: FileRun<'_>,
    cancel: &ScriptCanceller,
    tally: &mut Tally<E>,
) -> (Ended, bool) {
    if !run.transaction {
        return (
            statements(manager, session, run, cancel, tally).await,
            false,
        );
    }
    if let Err(message) = control(manager, session, "BEGIN").await {
        return (Ended::Stopped(message), false);
    }
    let ended = statements(manager, session, run, cancel, tally).await;
    if ended == Ended::Completed {
        return match control(manager, session, "COMMIT").await {
            Ok(()) => (Ended::Completed, false),
            Err(message) => (Ended::Stopped(message), true),
        };
    }
    // Best effort: a lost connection has already rolled the transaction back.
    let _ = control(manager, session, "ROLLBACK").await;
    (ended, true)
}

/// Run a transaction-control statement, even after the run was cancelled.
async fn control(manager: &SessionManager, session: SessionId, sql: &str) -> Result<(), String> {
    let statement = BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    };
    let mut failure = None;
    let outcome = run_bound(
        manager,
        session,
        statement,
        &ScriptCanceller::default(),
        |event| {
            if let QueryEvent::Failed { error } = event.event {
                failure = Some(error.message);
            }
        },
    )
    .await;
    match outcome {
        ScriptOutcome::Completed => Ok(()),
        _ => Err(format!(
            "{sql} failed: {}",
            failure.unwrap_or_else(|| "the statement did not complete".to_owned())
        )),
    }
}

/// Read and run every batch; see [`run_file`].
async fn statements<E: FnMut(FileEvent)>(
    manager: &SessionManager,
    session: SessionId,
    run: FileRun<'_>,
    cancel: &ScriptCanceller,
    tally: &mut Tally<E>,
) -> Ended {
    let (sender, mut batches) = mpsc::channel(READ_AHEAD);
    let path = run.path.to_path_buf();
    let dialect = run.policy.dialect;
    let reader = tokio::task::spawn_blocking(move || read(&path, dialect, &sender));
    let sink: Arc<dyn BatchSink> = Arc::new(Discard);
    let stop = run.stop_on_error || run.transaction;
    let mut ended = Ended::Completed;
    while let Some(read) = batches.recv().await {
        let batch = match read {
            Read::Batch(batch) => batch,
            Read::Failed(error) => {
                ended = Ended::Stopped(capitalised(&error.to_string()));
                break;
            }
        };
        if let Some(reason) = blocked(&batch, run.policy) {
            ended = Ended::Stopped(reason);
            break;
        }
        let outcome = run_batch(manager, session, (&batch, stop, &sink), cancel, tally).await;
        if let Some(end) = batch_end(outcome, stop || tally.lost()) {
            ended = end;
            break;
        }
    }
    // Dropping the receiver ends the reader at its next send.
    drop(batches);
    let _ = reader.await;
    if ended == Ended::Completed && tally.errors() > 0 {
        Ended::Failed
    } else {
        ended
    }
}

/// How a batch's outcome ends the run, if it does; failures end it only
/// when errors stop the run or the connection was lost.
fn batch_end(outcome: ScriptOutcome, stop: bool) -> Option<Ended> {
    match outcome {
        ScriptOutcome::Cancelled => Some(Ended::Cancelled),
        ScriptOutcome::Failed if stop => Some(Ended::Failed),
        ScriptOutcome::Completed | ScriptOutcome::Failed => None,
    }
}

/// One batch through the script runner, its events counted by `tally`.
async fn run_batch<E: FnMut(FileEvent)>(
    manager: &SessionManager,
    session: SessionId,
    (batch, stop_on_error, sink): (&[ScriptStatement], bool, &Arc<dyn BatchSink>),
    cancel: &ScriptCanceller,
    tally: &mut Tally<E>,
) -> ScriptOutcome {
    let bound: Vec<BoundSql> = batch
        .iter()
        .map(|statement| BoundSql {
            sql: statement.text.clone(),
            params: Vec::new(),
        })
        .collect();
    let script = Script {
        statements: &bound,
        max_rows: None,
        stop_on_error,
        sink: Some(sink),
    };
    tally.start_batch();
    run_script(manager, session, script, cancel, |event| {
        if let Some(statement) = batch.get(event.statement) {
            tally.observe((event.statement, statement), &event.event);
        }
    })
    .await
}

/// The first statement the connection refuses (the file changed since it
/// was checked, or no check ran).
fn blocked(batch: &[ScriptStatement], policy: SafetyPolicy) -> Option<String> {
    let statement = batch.iter().find(|statement| {
        verdict(policy, classify(&statement.text, policy.dialect)) == Verdict::Block
    })?;
    Some(format!(
        "This connection is read-only: the statement on line {} may modify the database ({})",
        statement.line,
        preview(&statement.text)
    ))
}

/// The reader thread: batches of statements until the end, an error or a
/// closed channel.
fn read(path: &Path, dialect: Dialect, sender: &mpsc::Sender<Read>) {
    let reader = match ScriptReader::open(path, dialect) {
        Ok(reader) => reader,
        Err(error) => {
            let _ = sender.blocking_send(Read::Failed(error));
            return;
        }
    };
    let mut batch = Vec::with_capacity(BATCH);
    for statement in reader {
        match statement {
            Ok(statement) => batch.push(statement),
            Err(error) => {
                let full = std::mem::take(&mut batch);
                if !full.is_empty() && sender.blocking_send(Read::Batch(full)).is_err() {
                    return;
                }
                let _ = sender.blocking_send(Read::Failed(error));
                return;
            }
        }
        if batch.len() == BATCH {
            let full = std::mem::replace(&mut batch, Vec::with_capacity(BATCH));
            if sender.blocking_send(Read::Batch(full)).is_err() {
                return;
            }
        }
    }
    if !batch.is_empty() {
        let _ = sender.blocking_send(Read::Batch(batch));
    }
}

fn capitalised(message: &str) -> String {
    let mut chars = message.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}
