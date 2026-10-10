// SPDX-License-Identifier: Apache-2.0
//! A cancellation identity is captured before enqueueing, never read by the host later.
use crate::{HostFailure, HostSession, HostState, session::Shutdown};
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use openrows_host_protocol::{Control, ControlQueue, Frame};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{mpsc, watch};

pub(crate) struct Cancellation {
    pub(crate) active: AtomicU64,
    pub(crate) cancelled: watch::Sender<u64>,
    generation: u64,
    outgoing: ControlQueue,
    shutdown: mpsc::Sender<Shutdown>,
    state: watch::Receiver<HostState>,
    failure: Arc<crate::failure_signal::FailureSignal>,
}
impl Cancellation {
    pub(crate) fn new(host: &HostSession) -> Arc<Self> {
        Arc::new(Self {
            active: AtomicU64::new(0),
            cancelled: watch::channel(0).0,
            generation: host.generation,
            outgoing: host.outgoing.clone(),
            shutdown: host.shutdown.clone(),
            state: host.subscribe(),
            failure: host.failure.clone(),
        })
    }
}
impl CancelHandle for Cancellation {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let target = self.active.load(Ordering::SeqCst);
        let result = if target == 0 {
            Ok(())
        } else {
            match *self.state.borrow() {
                HostState::Failed(error) => Err(error),
                HostState::Closed => Err(HostFailure::Exited),
                HostState::Ready => Frame::control(self.generation, 0, &Control::Cancel { target })
                    .map_err(HostFailure::from)
                    .and_then(|frame| self.outgoing.send(frame).map_err(HostFailure::from))
                    .and_then(|()| {
                        self.shutdown
                            .try_send(Shutdown::Drain(target))
                            .map_err(|_| HostFailure::Protocol)
                    }),
            }
        };
        if target != 0 && result.is_ok() {
            self.cancelled.send_replace(target);
        }
        if let Err(error) = result {
            self.failure.fail(error);
        }
        Box::pin(std::future::ready(result.map_err(DriverError::Host)))
    }
}
