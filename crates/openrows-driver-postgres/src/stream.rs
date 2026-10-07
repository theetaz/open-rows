// SPDX-License-Identifier: Apache-2.0
//! Execute-phase message loop: rows → decoder → sink, always draining to `ReadyForQuery`.
//!
//! The schema is published with the first batch rather than at `Describe`, so a
//! column that falls back to tagged text in the first batch (docs/05 §11) never
//! changes the schema the sink saw. A fallback after that starts a new result.
use crate::decode::ResultDecoder;
use crate::wire::{Wire, notice, server_error, unexpected};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ExecSummary, NoticeLevel, ResultSink, SinkFlow};
use postgres_protocol::message::backend::Message;
use std::sync::Arc;
use std::time::Instant;

/// Explains why one statement produced a second result set.
const CONTINUED: &str = "a column held values outside its Arrow type (e.g. infinity);     the remaining rows continue in a new result with that column as text";

/// Stream one statement's results; errors are reported only after the
/// connection is back at `ReadyForQuery`, so the session stays usable.
pub(crate) async fn collect(
    wire: &mut Wire,
    decoder: Option<ResultDecoder>,
    sink: &mut dyn ResultSink,
) -> Result<ExecSummary, DriverError> {
    let mut state = Collector {
        decoder,
        sink,
        summary: ExecSummary::default(),
        failure: None,
        stopped: false,
        first_row: None,
        published: None,
    };
    loop {
        match wire.next().await? {
            Message::DataRow(body) => state.row(body.buffer()),
            Message::CommandComplete(body) => state.complete(body.tag()?),
            Message::ErrorResponse(body) => state.fail(server_error(&body)),
            Message::NoticeResponse(body) => {
                let (level, message) = notice(&body);
                state.sink.notice(level, &message);
            }
            Message::ReadyForQuery(_) => return state.finish(),
            Message::ParseComplete
            | Message::BindComplete
            | Message::EmptyQueryResponse
            | Message::ParameterStatus(_) => {}
            _ => return Err(unexpected("execute")),
        }
    }
}

struct Collector<'a> {
    decoder: Option<ResultDecoder>,
    sink: &'a mut dyn ResultSink,
    summary: ExecSummary,
    failure: Option<DriverError>,
    stopped: bool,
    first_row: Option<Instant>,
    published: Option<SchemaRef>,
}

impl Collector<'_> {
    fn row(&mut self, row: &[u8]) {
        if self.failure.is_some() || self.stopped {
            return;
        }
        self.first_row.get_or_insert_with(Instant::now);
        let Some(decoder) = self.decoder.as_mut() else {
            self.fail(DriverError::Protocol(
                "DataRow without RowDescription".to_owned(),
            ));
            return;
        };
        match decoder.decode_row(row) {
            Ok(Some(batch)) => self.push(batch),
            Ok(None) => {}
            Err(error) => self.fail(error),
        }
    }

    fn complete(&mut self, tag: &str) {
        tag.clone_into(&mut self.summary.command_tag);
        if self.failure.is_some() || self.stopped {
            return;
        }
        match self.decoder.as_mut().map(ResultDecoder::finish) {
            Some(Ok(Some(batch))) => self.push(batch),
            Some(Err(error)) => self.fail(error),
            Some(Ok(None)) | None => {}
        }
        if self.published.is_none()
            && let Some(decoder) = &self.decoder
        {
            self.publish(decoder.schema());
        }
    }

    /// Start the result on first use and again whenever the schema changed.
    fn publish(&mut self, schema: SchemaRef) {
        match &self.published {
            Some(published) if Arc::ptr_eq(published, &schema) => return,
            Some(_) => self.sink.notice(NoticeLevel::Warning, CONTINUED),
            None => {}
        }
        self.sink.begin_result(schema.clone());
        self.published = Some(schema);
    }

    fn push(&mut self, batch: RecordBatch) {
        self.publish(batch.schema());
        if self.summary.batches == 0 {
            self.summary.first_batch_latency = self.first_row.map(|start| start.elapsed());
        }
        self.summary.rows += batch.num_rows() as u64;
        self.summary.batches += 1;
        self.stopped = self.sink.push_batch(batch) == SinkFlow::Stop;
    }

    fn fail(&mut self, error: DriverError) {
        self.failure.get_or_insert(error);
    }

    fn finish(self) -> Result<ExecSummary, DriverError> {
        self.failure.map_or(Ok(self.summary), Err)
    }
}
