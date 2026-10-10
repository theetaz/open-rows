// SPDX-License-Identifier: Apache-2.0
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;

/// Admission before another hosted batch; existing drivers do not wait on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinkAdmission {
    /// Space is available.
    Ready,
    /// Withhold credit until the sink signals space.
    Pause,
    /// Released result, row cap or failed spill.
    Stop,
}

/// Consumer decision after each batch (docs/06 §5 back-pressure).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinkFlow {
    /// Keep streaming.
    Continue,
    /// Stop decoding; the driver drains the remaining rows without decoding them.
    Stop,
}

/// Severity of a server notice forwarded to the user (docs/06 §4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeLevel {
    /// Informational message, e.g. `PostgreSQL` `NOTICE` or `INFO`.
    Info,
    /// Something the user should look at, e.g. `PostgreSQL` `WARNING`.
    Warning,
}

/// Outcome of one executed statement.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExecSummary {
    /// Rows delivered to the sink.
    pub rows: u64,
    /// Batches delivered to the sink.
    pub batches: u64,
    /// Server command tag, e.g. `SELECT 42`.
    pub command_tag: String,
    /// Time from the first data row arriving to the first batch reaching the sink.
    pub first_batch_latency: Option<std::time::Duration>,
}

/// Receives the columnar result of one statement.
pub trait ResultSink: Send {
    /// Called before the first batch of each result with its schema. A driver
    /// may call it again within one statement when a column's representation
    /// has to change mid-stream; later batches belong to the new result.
    fn begin_result(&mut self, schema: SchemaRef);
    /// Called for every batch in order.
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow;
    /// Nonblocking admission; defaults to today's unlimited streaming.
    fn admission(&self) -> SinkAdmission {
        SinkAdmission::Ready
    }
    /// Wait for admission; paused sinks override this to signal available space.
    fn resume(&mut self) -> crate::BoxFuture<'_, SinkAdmission> {
        match self.admission() {
            SinkAdmission::Pause => Box::pin(std::future::pending()),
            admission => Box::pin(std::future::ready(admission)),
        }
    }
    /// Admission stopped the stream before another batch could be accepted.
    fn stopped(&mut self) {}
    /// Called for each server notice raised while the statement runs.
    fn notice(&mut self, _level: NoticeLevel, _message: &str) {}
}

/// Drops every row: for internal statements whose results do not matter
/// (keepalive probes, replayed session settings).
#[derive(Clone, Copy, Debug, Default)]
pub struct DiscardSink;

impl ResultSink for DiscardSink {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Stop
    }
}
