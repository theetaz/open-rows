// SPDX-License-Identifier: Apache-2.0
//! Serial connection owner and its pre-acquired out-of-band native interrupt.
use crate::{flow::Flow, output::Output, sink::Sink};
use openrows_driver_api::{BoxFuture, CancelHandle, ConnectTrace, Connection, DriverError};
use openrows_host_protocol::{
    Connected, Control, Facets, ProtocolError, Reply, WireSummary, WireTrace,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

pub(crate) struct Active {
    request: u64,
    cancel: Option<Arc<dyn CancelHandle>>,
    pub(crate) pending: BTreeMap<u64, bool>,
}
impl Active {
    pub(crate) fn new() -> Self {
        Self {
            request: 0,
            cancel: None,
            pending: BTreeMap::new(),
        }
    }
    pub(crate) fn cancel(
        &mut self,
        target: u64,
    ) -> Option<BoxFuture<'static, Result<(), DriverError>>> {
        if self.request != target {
            if let Some(cancelled) = self.pending.get_mut(&target) {
                *cancelled = true;
            }
            return None;
        }
        self.cancel.as_ref().map(|cancel| cancel.cancel())
    }
}
pub(crate) struct Worker {
    pub(crate) commands: openrows_host_protocol::QueueReceiver,
    pub(crate) output: Output,
    pub(crate) flow: Arc<Flow>,
    pub(crate) active: Arc<Mutex<Active>>,
    pub(crate) authorized: Arc<AtomicU64>,
}
impl Worker {
    pub(crate) fn run(mut self) -> Result<(), ProtocolError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| ProtocolError::Transport)?;
        let mut connection: Option<Box<dyn Connection>> = None;
        while let Some(queued) = runtime.block_on(self.commands.recv()) {
            let frame = &queued.frame;
            if self.flow.closing() {
                break;
            }
            self.output.request = frame.header.request;
            let control = frame.decode_control()?;
            let execution = matches!(
                control,
                Control::Execute { .. } | Control::ExecuteBound { .. }
            );
            let cancelled = !execution
                && self
                    .active
                    .lock()
                    .map_err(|_| ProtocolError::Invalid)?
                    .pending
                    .remove(&frame.header.request)
                    .unwrap_or(false);
            let result = if cancelled {
                Err(cancelled_error())
            } else {
                runtime.block_on(self.dispatch(control, &mut connection))
            };
            self.active
                .lock()
                .map_err(|_| ProtocolError::Invalid)?
                .request = 0;
            let terminal = result.map_or_else(|error| Control::Error(error.into()), Control::Reply);
            self.output.control(0, &terminal)?;
        }
        if let Some(connection) = connection {
            runtime
                .block_on(connection.close())
                .map_err(|_| ProtocolError::Invalid)?;
        }
        Ok(())
    }
    async fn dispatch(
        &mut self,
        control: Control,
        connection: &mut Option<Box<dyn Connection>>,
    ) -> Result<Reply, DriverError> {
        match control {
            Control::Connect(config) => {
                if connection.is_some() {
                    return Err(DriverError::Unsupported("one connection per host".into()));
                }
                let request = config.to_request()?;
                let mut trace = ConnectTrace::default();
                let opened = crate::registry::connect(&request, &mut trace).await?;
                let summary = opened.server();
                let reply = Connected {
                    product: summary.product.clone(),
                    version: summary.version.clone(),
                    flavour: summary.flavour.clone(),
                    database: summary.database.clone(),
                    user: summary.user.clone(),
                    tls: 2,
                    trace: WireTrace::from(&trace),
                    facets: Facets::default(),
                    in_transaction: opened.in_transaction(),
                };
                self.active.lock().map_err(|_| protocol())?.cancel = opened.cancel_handle();
                *connection = Some(opened);
                Ok(Reply::Connected(reply))
            }
            Control::Execute { sql } => self.execute(connection, &sql, &[]).await,
            Control::ExecuteBound { sql, params } => self.execute(connection, &sql, &params).await,
            Control::Ping => {
                connection.as_mut().ok_or_else(protocol)?.ping().await?;
                Ok(Reply::Pong)
            }
            Control::Catalog { .. } | Control::Editor | Control::Admin | Control::Bulk => Err(
                DriverError::Unsupported("host facet pending M2-01.3".into()),
            ),
            _ => Err(protocol()),
        }
    }
    async fn execute(
        &mut self,
        connection: &mut Option<Box<dyn Connection>>,
        sql: &str,
        params: &[Option<String>],
    ) -> Result<Reply, DriverError> {
        let connection = connection.as_mut().ok_or_else(protocol)?;
        {
            let mut active = self.active.lock().map_err(|_| protocol())?;
            self.flow
                .begin(self.output.request)
                .map_err(|_| protocol())?;
            if active.pending.remove(&self.output.request).unwrap_or(false) {
                return Err(cancelled_error());
            }
            active.request = self.output.request;
        }
        if self.flow.cancelled() {
            return Err(cancelled_error());
        }
        #[cfg(feature = "test-hooks")]
        if let Some(result) = crate::hooks::execute(sql, &self.output, &self.flow, &self.authorized)
        {
            self.active.lock().map_err(|_| protocol())?.request = 0;
            return result;
        }
        let mut sink = Sink::new(
            self.output.clone(),
            self.flow.clone(),
            self.authorized.clone(),
        );
        let outcome = connection.execute_bound(sql, params, &mut sink).await;
        self.active.lock().map_err(|_| protocol())?.request = 0;
        let finished = sink.finish();
        if self.flow.cancelled() {
            return Err(cancelled_error());
        }
        if matches!(sink.error, Some(ProtocolError::Limit)) {
            return Err(DriverError::Unsupported(
                "host row exceeds 64 MiB encoded or decoded ceiling".into(),
            ));
        }
        if sink.error.is_some() || finished.is_err() {
            return Err(protocol());
        }
        let mut summary = WireSummary::from(outcome?);
        summary.in_transaction = connection.in_transaction();
        summary.result = sink.result;
        summary.sequence = sink.sequence;
        Ok(Reply::Executed(summary))
    }
}
fn protocol() -> DriverError {
    DriverError::Host(openrows_driver_api::HostFailure::Protocol)
}

pub(crate) fn start(
    worker: Worker,
) -> Result<tokio::sync::oneshot::Receiver<Result<(), ProtocolError>>, ProtocolError> {
    let (finished, stopped) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("host-connection".into())
        .spawn(move || {
            let _ = finished.send(worker.run());
        })
        .map_err(|_| ProtocolError::Transport)?;
    Ok(stopped)
}

fn cancelled_error() -> DriverError {
    DriverError::Server {
        code: "57014".into(),
        message: "statement cancelled".into(),
    }
}
