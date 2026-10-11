// SPDX-License-Identifier: Apache-2.0
//! Serial operation IDs, terminal replies and host-only lifecycle observation.
use crate::{HostFailure, HostState, HostedConnection};
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session};
use openrows_host_protocol::{Control, Reply};
use std::sync::{Arc, atomic::Ordering};
impl HostedConnection {
    pub(crate) fn begin(&mut self, control: &Control) -> Result<u64, DriverError> {
        let preflight = openrows_host_protocol::Frame::control(self.host.generation(), 0, control)
            .and_then(|frame| frame.decode_control());
        if preflight.is_err() {
            return Err(DriverError::Unsupported(
                "host request exceeds supported control limits".into(),
            ));
        }
        self.next = self.next.checked_add(1).ok_or_else(|| self.protocol())?;
        self.host
            .send(self.next, control)
            .map_err(DriverError::Host)?;
        self.result = 0;
        self.cancel.active.store(self.next, Ordering::SeqCst);
        Ok(self.next)
    }
    pub(crate) async fn receive(&mut self, request: u64) -> Result<Control, DriverError> {
        let frame = self.host.recv().await.map_err(DriverError::Host)?;
        let control = frame.decode_control().map_err(|_| self.protocol())?;
        if frame.header.request != request
            && !matches!(control, Control::CancelAck { .. } | Control::Paused { .. })
        {
            return Err(self.protocol());
        }
        self.validate_result(frame.header, &control)?;
        Ok(control)
    }
    fn validate_result(
        &mut self,
        header: openrows_host_protocol::Header,
        control: &Control,
    ) -> Result<(), DriverError> {
        match control {
            Control::ResultBegin => {
                self.result = self.result.checked_add(1).ok_or_else(|| self.protocol())?;
                if header.result != self.result {
                    return Err(self.protocol());
                }
            }
            Control::ResultEnd { .. } if header.result != self.result => {
                return Err(self.protocol());
            }
            Control::BatchReady(grant)
                if header.result != self.result || grant.key.result != self.result =>
            {
                return Err(self.protocol());
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) async fn call(&mut self, control: &Control) -> Result<Reply, DriverError> {
        let request = self.begin(control)?;
        self.finish_call(request).await
    }
    pub(crate) async fn finish_call(&mut self, request: u64) -> Result<Reply, DriverError> {
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(30), self.terminal(request)).await;
        self.cancel.active.store(0, Ordering::SeqCst);
        if let Ok(result) = result {
            result
        } else {
            self.host.poison(HostFailure::Unresponsive);
            Err(DriverError::Host(HostFailure::Unresponsive))
        }
    }
    async fn terminal(&mut self, request: u64) -> Result<Reply, DriverError> {
        let mut chunks = crate::adapter_facet::FacetChunks::default();
        loop {
            match self.receive(request).await? {
                Control::Accepted | Control::CancelAck { .. } | Control::Paused { .. } => {}
                Control::FacetChunk { sequence, bytes } => {
                    chunks
                        .push(sequence, &bytes)
                        .map_err(|()| self.protocol())?;
                    self.host
                        .send(request, &Control::FacetConsumed { sequence })
                        .map_err(DriverError::Host)?;
                }
                Control::Reply(Reply::FacetChunks {
                    sequence,
                    in_transaction,
                }) => {
                    let value = chunks.finish(sequence).map_err(|()| self.protocol())?;
                    return Ok(Reply::Facet {
                        value,
                        in_transaction,
                    });
                }
                Control::Reply(reply) if !chunks.started() => return Ok(reply),
                Control::ConnectError { error, trace } if request == 1 && !chunks.started() => {
                    self.failed_connect = Some(trace);
                    return Err(self.remote_error(error));
                }
                Control::Error(error) => return Err(self.remote_error(error)),
                _ => return Err(self.protocol()),
            }
        }
    }
}
impl Session for HostedConnection {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        self.execute_bound(sql, &[], sink)
    }
    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move {
            if self.duckdb {
                crate::duckdb::admit(sql)?;
            }
            let request = self.begin(&Control::ExecuteBound {
                sql: sql.into(),
                params: params.to_vec(),
            })?;
            let result = self.stream(request, sink).await;
            self.cancel.active.store(0, Ordering::SeqCst);
            result
        })
    }
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(self.cancel.clone())
    }
    fn host_failure(&self) -> Option<BoxFuture<'static, DriverError>> {
        Some(Box::pin(observe(self.host.subscribe())))
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move {
            match self.call(&Control::Ping).await? {
                Reply::Pong => Ok(()),
                _ => Err(self.protocol()),
            }
        })
    }
    fn in_transaction(&self) -> bool {
        self.transaction
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            let reservation = self.reservation;
            let result = self.host.close().await.map_err(DriverError::Host);
            drop(reservation);
            result
        })
    }
}

async fn observe(mut state: tokio::sync::watch::Receiver<HostState>) -> DriverError {
    loop {
        let current = *state.borrow_and_update();
        match current {
            HostState::Failed(error) => return DriverError::Host(error),
            HostState::Closed => return DriverError::Host(HostFailure::Exited),
            HostState::Ready => {}
        }
        if state.changed().await.is_err() {
            return DriverError::Host(HostFailure::Exited);
        }
    }
}
