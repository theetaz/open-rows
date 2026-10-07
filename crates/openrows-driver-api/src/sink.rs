// SPDX-License-Identifier: Apache-2.0
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;

/// Consumer decision after each batch (docs/06 §5 back-pressure).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinkFlow {
    /// Keep streaming.
    Continue,
    /// Stop decoding; the driver drains the remaining rows without decoding them.
    Stop,
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
    /// Called once, before any batch, with the result schema.
    fn begin_result(&mut self, schema: SchemaRef);
    /// Called for every batch in order.
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow;
}
