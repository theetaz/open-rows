// SPDX-License-Identifier: Apache-2.0
//! Execute-phase message loop: rows → decoder → sink, always draining to `ReadyForQuery`.
use crate::decode::ResultDecoder;
use crate::wire::{Wire, notice, server_error, unexpected};
use arrow_array::RecordBatch;
use openrows_driver_api::{DriverError, ExecSummary, ResultSink, SinkFlow};
use postgres_protocol::message::backend::Message;
use std::time::Instant;

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
    }

    fn push(&mut self, batch: RecordBatch) {
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
