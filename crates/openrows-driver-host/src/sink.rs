// SPDX-License-Identifier: Apache-2.0
//! Synchronous driver sink blocks only its dedicated connection worker.
use crate::{flow::Flow, output::Output};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{ResultSink, SinkFlow};
use openrows_host_protocol::{
    ArrowEncoder, BATCH_LIMIT, Control, EncodedBatch, FlowKey, Grant, ProtocolError, split_batch,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub(crate) struct Sink {
    pub(crate) output: Output,
    pub(crate) flow: Arc<Flow>,
    pub(crate) result: u32,
    pub(crate) sequence: u64,
    pub(crate) error: Option<ProtocolError>,
    encoder: Option<ArrowEncoder>,
    batch: u64,
    had_batch: bool,
    authorized: Arc<AtomicU64>,
}
impl Sink {
    pub(crate) fn new(output: Output, flow: Arc<Flow>, authorized: Arc<AtomicU64>) -> Self {
        Self {
            output,
            flow,
            result: 0,
            sequence: 0,
            error: None,
            encoder: None,
            batch: 0,
            had_batch: false,
            authorized,
        }
    }
    fn publish(&mut self, encoded: EncodedBatch) -> Result<bool, ProtocolError> {
        self.batch += 1;
        let grant = Grant {
            key: FlowKey {
                request: self.output.request,
                result: self.result,
                batch: self.batch,
            },
            encoded: u32::try_from(encoded.bytes.len()).map_err(|_| ProtocolError::Limit)?,
            decoded: u32::try_from(encoded.decoded).map_err(|_| ProtocolError::Limit)?,
            exclusive: encoded.bytes.len() > BATCH_LIMIT || encoded.decoded > BATCH_LIMIT,
        };
        self.flow.offer(grant)?;
        self.output
            .control(self.result, &Control::BatchReady(grant))?;
        if !self.flow.authorize()? {
            return Ok(false);
        }
        let count = encoded
            .bytes
            .len()
            .div_ceil(openrows_host_protocol::FRAGMENT_LIMIT) as u64;
        self.authorized
            .store(self.sequence + count, Ordering::SeqCst);
        self.output
            .data(self.result, &mut self.sequence, encoded.bytes)?;
        self.had_batch = true;
        self.flow.consumed()
    }
    fn push(&mut self, batch: &RecordBatch) -> Result<bool, ProtocolError> {
        for part in split_batch(batch)? {
            let part = part?;
            let encoded = self
                .encoder
                .as_mut()
                .ok_or(ProtocolError::Invalid)?
                .batch(&part)?;
            if !self.publish(encoded)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(crate) fn finish(&mut self) -> Result<(), ProtocolError> {
        if self.encoder.is_none() {
            return Ok(());
        }
        if self.flow.stopped() || self.flow.cancelled() || self.flow.closing() {
            self.encoder = None;
            return self.output.control(
                self.result,
                &Control::ResultEnd {
                    sequence: self.sequence,
                },
            );
        }
        if !self.had_batch && !self.flow.cancelled() && !self.flow.closing() {
            let encoded = self
                .encoder
                .as_mut()
                .ok_or(ProtocolError::Invalid)?
                .schema_only();
            if !self.publish(encoded)? {
                self.encoder = None;
                return Ok(());
            }
        }
        let eos = self
            .encoder
            .as_mut()
            .ok_or(ProtocolError::Invalid)?
            .finish()?;
        if self.had_batch && !self.flow.cancelled() && !self.flow.closing() {
            self.output.data(self.result, &mut self.sequence, eos)?;
        }
        self.output.control(
            self.result,
            &Control::ResultEnd {
                sequence: self.sequence,
            },
        )?;
        self.encoder = None;
        Ok(())
    }
}
impl ResultSink for Sink {
    fn begin_result(&mut self, schema: SchemaRef) {
        let result = self.finish().and_then(|()| {
            self.result = self.result.checked_add(1).ok_or(ProtocolError::Limit)?;
            self.sequence = 0;
            self.authorized.store(0, Ordering::SeqCst);
            self.batch = 0;
            self.had_batch = false;
            self.encoder = Some(ArrowEncoder::new(schema)?);
            self.output.control(self.result, &Control::ResultBegin)
        });
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.error.is_some() {
            return SinkFlow::Stop;
        }
        match self.push(&batch) {
            Ok(true) => SinkFlow::Continue,
            Ok(false) => SinkFlow::Stop,
            Err(error) => {
                self.error = Some(error);
                SinkFlow::Stop
            }
        }
    }
}
