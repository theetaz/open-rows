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
    pub(crate) registry: crate::registry::Registry,
    pub(crate) commands: openrows_host_protocol::QueueReceiver,
    pub(crate) output: Output,
    pub(crate) flow: Arc<Flow>,
    pub(crate) active: Arc<Mutex<Active>>,
    pub(crate) authorized: Arc<AtomicU64>,
    pub(crate) failed_connect: Option<WireTrace>,
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
            {
                let mut active = self.active.lock().map_err(|_| ProtocolError::Invalid)?;
                self.flow.begin(frame.header.request)?;
                active.request = frame.header.request;
            }
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
            self.failed_connect = None;
            let result = if cancelled {
                Err(cancelled_error())
            } else {
                runtime.block_on(self.dispatch(control, &mut connection))
            };
            let terminal = match result {
                Ok(reply) => Control::Reply(reply),
                Err(error) => match self.failed_connect.take() {
                    Some(trace) => Control::ConnectError {
                        error: error.into(),
                        trace,
                    },
                    None => Control::Error(error.into()),
                },
            };
            self.terminal(terminal)?;
            self.active
                .lock()
                .map_err(|_| ProtocolError::Invalid)?
                .request = 0;
        }
        if let Some(connection) = connection {
            runtime
                .block_on(connection.close())
                .map_err(|_| ProtocolError::Invalid)?;
        }
        Ok(())
    }
    fn terminal(&self, terminal: Control) -> Result<(), ProtocolError> {
        let terminal = if let Control::Reply(Reply::Facet {
            value,
            in_transaction,
        }) = terminal
        {
            match crate::facet_output::send(&self.output, &self.flow, value, in_transaction) {
                Ok(reply) => Control::Reply(reply),
                Err(error) => Control::Error(error.into()),
            }
        } else {
            terminal
        };
        match self.output.control(0, &terminal) {
            Ok(()) => Ok(()),
            Err(ProtocolError::Limit) => self.output.control(
                0,
                &Control::Error(openrows_host_protocol::WireError::Unsupported(
                    "host reply exceeds supported limit".into(),
                )),
            ),
            Err(error) => Err(error),
        }
    }
    async fn dispatch(
        &mut self,
        control: Control,
        connection: &mut Option<Box<dyn Connection>>,
    ) -> Result<Reply, DriverError> {
        match control {
            Control::Connect(config) => self.connect(config, connection).await,
            Control::Execute { sql } => self.execute(connection, &sql, &[]).await,
            Control::ExecuteBound { sql, params } => self.execute(connection, &sql, &params).await,
            Control::Ping => {
                connection.as_mut().ok_or_else(protocol)?.ping().await?;
                Ok(Reply::Pong)
            }
            Control::CatalogRead(request) => {
                crate::facets::catalog(connection.as_mut().ok_or_else(protocol)?.as_mut(), request)
                    .await
            }
            Control::Edit(request) => {
                crate::facets::edit(connection.as_mut().ok_or_else(protocol)?.as_mut(), request)
                    .await
            }
            Control::Catalog { .. } | Control::Editor | Control::Admin | Control::Bulk => Err(
                DriverError::Unsupported("host facet is not supported".into()),
            ),
            _ => Err(protocol()),
        }
    }
    async fn connect(
        &mut self,
        config: openrows_host_protocol::OwnedConnect,
        connection: &mut Option<Box<dyn Connection>>,
    ) -> Result<Reply, DriverError> {
        if connection.is_some() {
            return Err(DriverError::Unsupported("one connection per host".into()));
        }
        let request = config.to_request()?;
        let mut trace = ConnectTrace::default();
        let mut opened =
            match (self.registry.connect)(self.registry.driver, &request, &mut trace).await {
                Ok(opened) => opened,
                Err(error) => {
                    self.failed_connect = Some(WireTrace::from(&trace));
                    return Err(error);
                }
            };
        let facets = Facets {
            catalog: opened.catalog().is_some(),
            editor: opened.editor().is_some(),
            admin: false,
            bulk: false,
        };
        let summary = opened.server();
        let reply = Connected {
            product: summary.product.clone(),
            version: summary.version.clone(),
            flavour: summary.flavour.clone(),
            database: summary.database.clone(),
            user: summary.user.clone(),
            tls: 2,
            trace: WireTrace::from(&trace),
            facets,
            in_transaction: opened.in_transaction(),
        };
        self.active.lock().map_err(|_| protocol())?.cancel = opened.cancel_handle();
        *connection = Some(opened);
        Ok(Reply::Connected(reply))
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
