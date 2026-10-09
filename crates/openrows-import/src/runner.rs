// SPDX-License-Identifier: Apache-2.0
//! Executes a [`LoadPlan`]: transaction control, the fast path or batched
//! `INSERT`s, existing-key probes, cancellation and rollback.
use std::future::Future;
use std::sync::Arc;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    BulkTarget, CancelHandle, Connection, DriverError, ExecSummary, ResultSink, SinkFlow, TextRow,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::ImportError;
use crate::feed::{Batch, Chunk};
use crate::load::{
    BadRows, Failure, LoadPlan, MAX_REPORTED_ERRORS, Outcome, RunMode, Summary, TransactionMode,
};
use crate::sql::{ConflictPolicy, Statements, max_params};

/// Most rows per generated `INSERT`.
const ROWS_PER_INSERT: usize = 1_000;
/// Cell text bytes after which an `INSERT` is sent early (`MySQL` refuses
/// prepared parameters beyond 16 MiB).
const INSERT_BYTES: usize = 4 * 1024 * 1024;
/// Most keys per existing-key probe.
const PROBE_KEYS: usize = 1_000;

/// Keeps reading so the driver counts every row; stores nothing.
struct CountSink;

impl ResultSink for CountSink {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Continue
    }
}

/// The row count of a command tag such as `INSERT 0 5` or `COPY 5`.
fn affected(summary: &ExecSummary) -> Option<u64> {
    summary.command_tag.rsplit(' ').next()?.parse().ok()
}

/// Await `work`; on cancel, ask the server to abort it and await its end.
async fn guarded<T>(
    work: impl Future<Output = Result<T, DriverError>>,
    cancel: &CancellationToken,
    canceller: Option<&Arc<dyn CancelHandle>>,
) -> Result<T, DriverError> {
    tokio::pin!(work);
    tokio::select! {
        biased;
        result = &mut work => result,
        () = cancel.cancelled() => {
            if let Some(canceller) = canceller {
                // A failed cancel request still lets the statement finish.
                let _ = canceller.cancel().await;
            }
            work.await
        }
    }
}

enum Ended {
    Done,
    Cancelled,
}

pub(crate) struct Runner<'a> {
    connection: &'a mut dyn Connection,
    plan: &'a LoadPlan,
    statements: Statements,
    cancel: CancellationToken,
    canceller: Option<Arc<dyn CancelHandle>>,
    /// Key column positions in a row, when a dry run can probe keys.
    probe: Option<Vec<usize>>,
    summary: Summary,
    /// Rows written and skipped in the open single transaction.
    pending: (u64, u64),
    in_transaction: bool,
    created: bool,
}

impl<'a> Runner<'a> {
    pub(crate) fn new(
        connection: &'a mut dyn Connection,
        plan: &'a LoadPlan,
        cancel: CancellationToken,
    ) -> Result<Self, ImportError> {
        let statements = Statements::new(
            plan.dialect,
            &plan.table,
            (&plan.columns, &plan.key),
            plan.conflict,
        )?;
        let probe = (plan.mode == RunMode::Check && plan.create.is_none() && !plan.key.is_empty())
            .then(|| {
                plan.key
                    .iter()
                    .map(|key| plan.columns.iter().position(|column| column == key))
                    .collect::<Option<Vec<_>>>()
            })
            .flatten();
        let summary = Summary {
            conflicts: probe.as_ref().map(|_| 0),
            ..Summary::default()
        };
        Ok(Self {
            canceller: connection.cancel_handle(),
            connection,
            plan,
            statements,
            cancel,
            probe,
            summary,
            pending: (0, 0),
            in_transaction: false,
            created: false,
        })
    }
}

impl Runner<'_> {
    pub(crate) async fn run(
        mut self,
        mut feed: mpsc::Receiver<Chunk>,
        mut progress: Box<dyn FnMut(&Summary) + Send>,
    ) -> Outcome {
        let ended = self.drive(&mut feed, progress.as_mut()).await;
        // Stop the reader before rolling back.
        drop(feed);
        let outcome = self.finish(ended).await;
        let (Outcome::Finished(summary)
        | Outcome::Cancelled(summary)
        | Outcome::Failed(_, summary)) = &outcome;
        progress(summary);
        outcome
    }

    async fn drive(
        &mut self,
        feed: &mut mpsc::Receiver<Chunk>,
        progress: &mut (dyn FnMut(&Summary) + Send),
    ) -> Result<Ended, Failure> {
        if self.cancel.is_cancelled() {
            return Ok(Ended::Cancelled);
        }
        if self.plan.mode == RunMode::Import {
            self.prepare().await.map_err(Failure::Setup)?;
        }
        loop {
            let chunk = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Ok(Ended::Cancelled),
                chunk = feed.recv() => chunk,
            };
            match chunk {
                Some(Chunk::Batch(batch)) => self.batch(batch).await?,
                Some(Chunk::End(Ok(()))) => return Ok(Ended::Done),
                Some(Chunk::End(Err(error))) => return Err(Failure::File(error)),
                None => {
                    return Err(Failure::File(ImportError::Invalid(
                        "The file reader stopped unexpectedly".to_owned(),
                    )));
                }
            }
            progress(&self.summary);
        }
    }

    /// Run one statement, cancelled out of band when the load is.
    async fn exec(
        &mut self,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<ExecSummary, DriverError> {
        let Self {
            connection,
            cancel,
            canceller,
            ..
        } = self;
        let mut sink = CountSink;
        let work = connection.execute_bound(sql, params, &mut sink);
        guarded(work, cancel, canceller.as_ref()).await
    }

    /// Create the table, open the single transaction and truncate.
    async fn prepare(&mut self) -> Result<(), DriverError> {
        let single = self.plan.transaction == TransactionMode::Single;
        let ddl_commits = self.statements.ddl_commits();
        if let Some(create) = &self.plan.create
            && ddl_commits
        {
            self.exec(create, &[]).await?;
            self.created = true;
        }
        if single {
            self.exec(self.statements.begin(), &[]).await?;
            self.in_transaction = true;
        }
        if let Some(create) = &self.plan.create
            && !ddl_commits
        {
            self.exec(create, &[]).await?;
            self.created = true;
        }
        if self.plan.truncate && self.plan.create.is_none() {
            let truncate = self.statements.truncate();
            self.exec(&truncate, &[]).await?;
        }
        Ok(())
    }

    async fn batch(&mut self, batch: Batch) -> Result<(), Failure> {
        let span = batch.span();
        let Batch { rows, errors, .. } = batch;
        self.summary.rows += (rows.len() + errors.len()) as u64;
        self.summary.failed += errors.len() as u64;
        let abort = !errors.is_empty() && self.plan.bad_rows == BadRows::Abort;
        let room = MAX_REPORTED_ERRORS.saturating_sub(self.summary.errors.len());
        self.summary.errors.extend(errors.into_iter().take(room));
        if abort {
            return Err(Failure::BadRow);
        }
        if rows.is_empty() {
            return Ok(());
        }
        let result = match self.plan.mode {
            RunMode::Check => self.probe(&rows).await,
            RunMode::Import => self.write(rows).await,
        };
        result.map_err(|error| Failure::Batch { error, lines: span })
    }

    async fn write(&mut self, rows: Vec<TextRow>) -> Result<(), DriverError> {
        let count = rows.len() as u64;
        let per_batch = self.plan.transaction == TransactionMode::PerBatch;
        if per_batch {
            self.exec(self.statements.begin(), &[]).await?;
            self.in_transaction = true;
        }
        let stored = self.insert(rows).await?;
        let written = match self.plan.conflict {
            ConflictPolicy::Update => count,
            ConflictPolicy::Error | ConflictPolicy::Skip => stored.min(count),
        };
        if per_batch {
            self.exec("COMMIT", &[]).await?;
            self.in_transaction = false;
        } else {
            self.pending.0 += written;
            self.pending.1 += count - written;
        }
        self.summary.inserted += written;
        self.summary.skipped += count - written;
        Ok(())
    }

    /// The bulk loader for plain appends, else batched `INSERT`s.
    async fn insert(&mut self, rows: Vec<TextRow>) -> Result<u64, DriverError> {
        if self.plan.conflict == ConflictPolicy::Error {
            let Self {
                connection,
                plan,
                cancel,
                canceller,
                ..
            } = self;
            if let Some(loader) = connection.bulk_loader() {
                let target = BulkTarget {
                    schema: &plan.table.schema,
                    table: &plan.table.table,
                    columns: &plan.columns,
                };
                return guarded(loader.load(target, rows), cancel, canceller.as_ref()).await;
            }
        }
        self.insert_statements(rows).await
    }

    async fn insert_statements(&mut self, rows: Vec<TextRow>) -> Result<u64, DriverError> {
        let per_statement = self.statements.rows_per_insert(ROWS_PER_INSERT);
        let (mut params, mut count, mut bytes, mut stored) = (Vec::new(), 0, 0, 0);
        for row in rows {
            bytes += row.iter().flatten().map(String::len).sum::<usize>();
            params.extend(row);
            count += 1;
            if count == per_statement || bytes >= INSERT_BYTES {
                stored += self.insert_chunk(count, &params).await?;
                (count, bytes) = (0, 0);
                params.clear();
            }
        }
        if count > 0 {
            stored += self.insert_chunk(count, &params).await?;
        }
        Ok(stored)
    }

    async fn insert_chunk(
        &mut self,
        rows: usize,
        params: &[Option<String>],
    ) -> Result<u64, DriverError> {
        let sql = self.statements.insert(rows);
        let summary = self.exec(&sql, params).await?;
        Ok(affected(&summary).unwrap_or(rows as u64))
    }

    /// Count rows whose key exists in the table (dry run).
    async fn probe(&mut self, rows: &[TextRow]) -> Result<(), DriverError> {
        let Some(positions) = self.probe.clone() else {
            return Ok(());
        };
        let per_probe = (max_params(self.plan.dialect) / positions.len().max(1)).min(PROBE_KEYS);
        let keys: Vec<Vec<Option<String>>> = rows
            .iter()
            .map(|row| {
                positions
                    .iter()
                    .map(|&i| row.get(i).cloned().flatten())
                    .collect::<Vec<_>>()
            })
            .filter(|key| key.iter().all(Option::is_some))
            .collect();
        for chunk in keys.chunks(per_probe.max(1)) {
            let sql = self.statements.existing_keys(chunk.len());
            let params: Vec<Option<String>> = chunk.iter().flatten().cloned().collect();
            let found = self.exec(&sql, &params).await?.rows;
            *self.summary.conflicts.get_or_insert(0) += found;
        }
        Ok(())
    }

    async fn finish(mut self, ended: Result<Ended, Failure>) -> Outcome {
        let cancelled = self.cancel.is_cancelled();
        match ended {
            Ok(Ended::Done) if !cancelled => match self.commit().await {
                Ok(()) => Outcome::Finished(self.summary),
                Err(failure) => Outcome::Failed(failure, self.summary),
            },
            Ok(_) => {
                self.rollback().await;
                Outcome::Cancelled(self.summary)
            }
            Err(_) if cancelled => {
                self.rollback().await;
                Outcome::Cancelled(self.summary)
            }
            Err(failure) => {
                self.rollback().await;
                Outcome::Failed(failure, self.summary)
            }
        }
    }

    async fn commit(&mut self) -> Result<(), Failure> {
        if !self.in_transaction {
            return Ok(());
        }
        match self.connection.execute("COMMIT", &mut CountSink).await {
            Ok(_) => {
                self.in_transaction = false;
                Ok(())
            }
            Err(error) => {
                if matches!(error, DriverError::Server { .. }) {
                    self.rollback().await;
                }
                Err(Failure::Commit(error))
            }
        }
    }

    /// Undo uncommitted work, without the cancel guard; a `MySQL` table the
    /// single transaction created is dropped again. Failures are ignored:
    /// closing the dedicated connection rolls back too.
    async fn rollback(&mut self) {
        let single = self.plan.transaction == TransactionMode::Single;
        if self.in_transaction {
            let _ = self.connection.execute("ROLLBACK", &mut CountSink).await;
            self.in_transaction = false;
        }
        if single {
            self.summary.inserted -= self.pending.0;
            self.summary.skipped -= self.pending.1;
            self.pending = (0, 0);
            if self.created && self.statements.ddl_commits() {
                let _ = self
                    .connection
                    .execute(&self.statements.drop_table(), &mut CountSink)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_tags_report_affected_rows() {
        let tag = |text: &str| ExecSummary {
            command_tag: text.to_owned(),
            ..ExecSummary::default()
        };
        assert_eq!(affected(&tag("INSERT 0 5")), Some(5));
        assert_eq!(affected(&tag("INSERT 3")), Some(3));
        assert_eq!(affected(&tag("COPY 12")), Some(12));
        assert_eq!(affected(&tag("BEGIN")), None);
    }
}
