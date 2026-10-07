// SPDX-License-Identifier: Apache-2.0
//! Scripted in-memory driver session and result sink for actor tests.
use crate::BatchSink;
use arrow_array::{Int64Array, RecordBatch};
use arrow_schema::{ArrowError, DataType, Field, Schema, SchemaRef};
use openrows_driver_api::{
    BoxFuture, CancelHandle, DriverError, ExecSummary, NoticeLevel, ResultSink, Session, SinkFlow,
};
use openrows_types::ResultId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::Notify;

/// Counts cancel requests and wakes a hanging statement.
#[derive(Default)]
pub(super) struct FakeCancel {
    pub(super) requests: AtomicUsize,
    wake: Notify,
}

impl CancelHandle for FakeCancel {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        self.wake.notify_one();
        Box::pin(async { Ok(()) })
    }
}

/// Executes a tiny command language instead of SQL, e.g. `rows 3 10` or `hang`.
pub(super) struct Fake {
    pub(super) cancel: Option<Arc<FakeCancel>>,
}

impl Fake {
    pub(super) fn boxed(cancel: Option<Arc<FakeCancel>>) -> Box<dyn Session> {
        Box::new(Self { cancel })
    }

    async fn run(&self, sql: &str, sink: &mut dyn ResultSink) -> Result<ExecSummary, DriverError> {
        let words: Vec<&str> = sql.split_ascii_whitespace().collect();
        let number = |index: usize| {
            words
                .get(index)
                .and_then(|w| w.parse::<u64>().ok())
                .unwrap_or(1)
        };
        let mut summary = ExecSummary::default();
        match words.first().copied().unwrap_or_default() {
            "rows" => stream(sink, number(1), number(2), &mut summary)?,
            "fail" => return Err(server("42601", "syntax error")),
            "drop" => return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into()),
            "panic" => panic_now(),
            "hang" => self.hang().await?,
            "notice" => sink.notice(NoticeLevel::Warning, "careful"),
            "update" => "UPDATE 3".clone_into(&mut summary.command_tag),
            "sleep" => tokio::time::sleep(Duration::from_millis(number(1))).await,
            other => return Err(DriverError::Unsupported(other.to_owned())),
        }
        Ok(summary)
    }

    /// Wait until cancelled server-side, like `SELECT pg_sleep(...)`.
    async fn hang(&self) -> Result<(), DriverError> {
        match &self.cancel {
            Some(cancel) => cancel.wake.notified().await,
            None => std::future::pending().await,
        }
        Err(server("57014", "canceling statement due to user request"))
    }
}

#[expect(
    clippy::panic,
    reason = "simulates a driver bug for panic isolation tests"
)]
pub(super) fn panic_now() {
    panic!("fake driver bug");
}

fn server(code: &str, message: &str) -> DriverError {
    DriverError::Server {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

fn stream(
    sink: &mut dyn ResultSink,
    batches: u64,
    rows: u64,
    summary: &mut ExecSummary,
) -> Result<(), ArrowError> {
    let schema: SchemaRef = Arc::new(Schema::new(vec![Field::new("n", DataType::Int64, false)]));
    sink.begin_result(schema.clone());
    for _ in 0..batches {
        let values = Int64Array::from_iter_values(0..i64::try_from(rows).unwrap_or(0));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(values)])?;
        summary.rows += rows;
        summary.batches += 1;
        if sink.push_batch(batch) == SinkFlow::Stop {
            break;
        }
    }
    summary.command_tag = format!("SELECT {}", summary.rows);
    Ok(())
}

impl Session for Fake {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, sink))
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        self.cancel
            .clone()
            .map(|cancel| cancel as Arc<dyn CancelHandle>)
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}

/// Records batch counts per result; optionally stops after `limit` batches.
#[derive(Default)]
pub(super) struct Store {
    limit: Option<usize>,
    batches: Mutex<Vec<ResultId>>,
    ended: Mutex<Vec<ResultId>>,
}

impl Store {
    pub(super) fn limited(limit: usize) -> Self {
        Self {
            limit: Some(limit),
            ..Self::default()
        }
    }

    pub(super) fn batches(&self) -> usize {
        self.batches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    pub(super) fn ended(&self) -> usize {
        self.ended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl BatchSink for Store {
    fn begin(&self, _result: ResultId, _schema: SchemaRef) {}

    fn push(&self, result: ResultId, _batch: RecordBatch) -> SinkFlow {
        let mut batches = self.batches.lock().unwrap_or_else(PoisonError::into_inner);
        batches.push(result);
        if self.limit.is_some_and(|limit| batches.len() >= limit) {
            SinkFlow::Stop
        } else {
            SinkFlow::Continue
        }
    }

    fn end(&self, result: ResultId) {
        self.ended
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(result);
    }
}
