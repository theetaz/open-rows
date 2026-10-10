// SPDX-License-Identifier: Apache-2.0
//! Only the admitted batch is read/decoded; there is no background row reader.
use crate::HostedConnection;
use openrows_driver_api::{DriverError, ExecSummary, ResultSink, SinkAdmission, SinkFlow};
use openrows_host_protocol::{ArrowDecoder, Control, FlowKey, Grant, Reply, RequestTracker};
use std::time::Duration;

struct ResultStream {
    decoder: ArrowDecoder,
    ordinal: u32,
    sequence: u64,
    batch: u64,
    begun: bool,
    ended: bool,
    stopped: bool,
}
impl ResultStream {
    fn new(ordinal: u32) -> Self {
        Self {
            decoder: ArrowDecoder::new(),
            ordinal,
            sequence: 0,
            batch: 0,
            begun: false,
            ended: false,
            stopped: false,
        }
    }
}
impl HostedConnection {
    pub(crate) async fn stream(
        &mut self,
        request: u64,
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let mut result = ResultStream::new(0);
        let mut tracker = RequestTracker::new(self.host.generation());
        tracker
            .begin(self.host.generation(), request)
            .map_err(|_| self.protocol())?;
        loop {
            match self.receive(request).await? {
                Control::Accepted | Control::CancelAck { .. } | Control::Paused { .. } => {}
                Control::Notice { level, message } => {
                    let level = match level {
                        1 => openrows_driver_api::NoticeLevel::Info,
                        2 => openrows_driver_api::NoticeLevel::Warning,
                        _ => return Err(self.protocol()),
                    };
                    sink.notice(level, &message);
                }
                Control::ResultBegin => self.begin_result(request, &mut result, &mut tracker)?,
                Control::BatchReady(grant) => {
                    self.batch(request, grant, &mut result, (&mut tracker, sink))
                        .await?;
                }
                Control::ResultEnd { sequence } => {
                    self.end(sequence, &mut result, &mut tracker).await?;
                }
                Control::Reply(Reply::Executed(summary)) => {
                    return self.summary(request, summary, &result, &mut tracker);
                }
                Control::Error(error) => return Err(self.remote_error(error)),
                _ => return Err(self.protocol()),
            }
        }
    }
    fn begin_result(
        &self,
        request: u64,
        result: &mut ResultStream,
        tracker: &mut RequestTracker,
    ) -> Result<(), DriverError> {
        if result.ordinal != 0 && !result.ended {
            return Err(self.protocol());
        }
        *result = ResultStream::new(
            result
                .ordinal
                .checked_add(1)
                .ok_or_else(|| self.protocol())?,
        );
        tracker
            .result_begin(request, result.ordinal)
            .map_err(|_| self.protocol())
    }
    fn summary(
        &mut self,
        request: u64,
        summary: openrows_host_protocol::WireSummary,
        result: &ResultStream,
        tracker: &mut RequestTracker,
    ) -> Result<ExecSummary, DriverError> {
        if result.ordinal != summary.result || (result.ordinal != 0 && !result.ended) {
            return Err(self.protocol());
        }
        tracker
            .finish(request, (summary.result, summary.sequence))
            .map_err(|_| self.protocol())?;
        self.transaction = summary.in_transaction;
        Ok(ExecSummary {
            rows: summary.rows,
            batches: summary.batches,
            command_tag: summary.tag,
            first_batch_latency: summary.first_batch_ns.map(Duration::from_nanos),
        })
    }
    async fn batch(
        &mut self,
        request: u64,
        grant: Grant,
        result: &mut ResultStream,
        (tracker, sink): (&mut RequestTracker, &mut dyn ResultSink),
    ) -> Result<(), DriverError> {
        if result.ended
            || result.ordinal == 0
            || grant.key
                != (FlowKey {
                    request,
                    result: result.ordinal,
                    batch: result.batch + 1,
                })
        {
            return Err(self.protocol());
        }
        let mut credit = openrows_host_protocol::CreditState::default();
        credit.offer(grant).map_err(|_| self.protocol())?;
        result.batch += 1;
        let admission = self.admission(grant.key, sink).await?;
        if admission == SinkAdmission::Stop {
            sink.stopped();
            result.stopped = true;
            return self
                .host
                .send(0, &Control::Stop(grant.key))
                .map_err(DriverError::Host);
        }
        self.host
            .send(
                0,
                &if admission == SinkAdmission::Pause {
                    Control::Resume(grant)
                } else {
                    Control::Grant(grant)
                },
            )
            .map_err(DriverError::Host)?;
        let _window = self.metrics.admit(grant);
        let bytes = self.data(grant.encoded as usize, result, tracker).await?;
        let batch = result
            .decoder
            .admit(&bytes, grant)
            .map_err(|_| self.protocol())?;
        if !result.begun {
            sink.begin_result(result.decoder.schema().ok_or_else(|| self.protocol())?);
            result.begun = true;
        }
        let flow = batch.map_or(SinkFlow::Continue, |batch| sink.push_batch(batch));
        if flow == SinkFlow::Stop {
            result.stopped = true;
            self.host
                .send(0, &Control::Stop(grant.key))
                .map_err(DriverError::Host)?;
        }
        self.host
            .send(0, &Control::Consumed(grant.key))
            .map_err(DriverError::Host)
    }
    async fn data(
        &mut self,
        size: usize,
        result: &mut ResultStream,
        tracker: &mut RequestTracker,
    ) -> Result<Vec<u8>, DriverError> {
        let mut bytes = Vec::with_capacity(size);
        while bytes.len() < size {
            let frame = self.host.read_data().await.map_err(DriverError::Host)?;
            if frame.header.result != result.ordinal || frame.body.len() > size - bytes.len() {
                return Err(self.protocol());
            }
            tracker.data(frame.header).map_err(|_| self.protocol())?;
            result.sequence = frame.header.sequence;
            bytes.extend_from_slice(&frame.body);
        }
        Ok(bytes)
    }
    async fn end(
        &mut self,
        sequence: u64,
        result: &mut ResultStream,
        tracker: &mut RequestTracker,
    ) -> Result<(), DriverError> {
        if result.ended || sequence < result.sequence {
            return Err(self.protocol());
        }
        if sequence != result.sequence {
            let eos = self.data(8, result, tracker).await?;
            result.decoder.finish(&eos).map_err(|_| self.protocol())?;
        } else if !result.stopped && *self.cancel.cancelled.borrow() != self.next {
            return Err(self.protocol());
        }
        if sequence != result.sequence {
            return Err(self.protocol());
        }
        result.ended = true;
        Ok(())
    }
    async fn admission(
        &mut self,
        key: FlowKey,
        sink: &mut dyn ResultSink,
    ) -> Result<SinkAdmission, DriverError> {
        let current = sink.admission();
        if current != SinkAdmission::Pause {
            return Ok(current);
        }
        self.host
            .send(0, &Control::Pause(key))
            .map_err(DriverError::Host)?;
        let mut cancelled = self.cancel.cancelled.subscribe();
        loop {
            if *cancelled.borrow_and_update() == key.request {
                return Ok(SinkAdmission::Stop);
            }
            tokio::select! {
                admission = sink.resume() => match admission {
                    SinkAdmission::Ready => return Ok(SinkAdmission::Pause),
                    SinkAdmission::Stop => return Ok(SinkAdmission::Stop),
                    SinkAdmission::Pause => tokio::task::yield_now().await,
                },
                changed = cancelled.changed() => { if changed.is_err() { return Err(self.protocol()); } },
                control = self.receive(key.request) => match control? {
                    Control::Paused { key: paused, .. } if paused == key => {},
                    Control::CancelAck { .. } => {},
                    Control::Error(error) => return Err(self.remote_error(error)),
                    _ => return Err(self.protocol()),
                }
            }
        }
    }
}
