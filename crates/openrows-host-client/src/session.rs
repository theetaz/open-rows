// SPDX-License-Identifier: Apache-2.0
//! Raw hosted-session channels and lifecycle subscription for the next adapter.
use crate::{HostFailure, LargeReservation, MemoryBudget, transport::Stream};
use openrows_host_protocol::{
    Control, ControlQueue, FlowKey, Frame, Grant, Lane, QueueReceiver, read_frame,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

/// Final failure is published only after targeted reap and endpoint cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostState {
    /// Authenticated and live.
    Ready,
    /// Successfully closed and reaped.
    Closed,
    /// Permanently poisoned; explicit new connect is required.
    Failed(HostFailure),
}
#[derive(Clone, Copy)]
pub(crate) enum Shutdown {
    Close,
    Failure(HostFailure),
    Drain(u64),
    Completed(u64),
}
pub(crate) type LargeGrant = Arc<Mutex<Option<(FlowKey, LargeReservation)>>>;
/// One process/connection lifetime; Drop initiates targeted cleanup without blocking.
pub struct HostSession {
    pub(crate) generation: u64,
    pub(crate) pid: u32,
    pub(crate) directory: PathBuf,
    pub(crate) policy: &'static str,
    pub(crate) outgoing: ControlQueue,
    pub(crate) incoming: QueueReceiver,
    pub(crate) data: Stream,
    pub(crate) state: watch::Receiver<HostState>,
    pub(crate) heartbeat: watch::Receiver<tokio::time::Instant>,
    pub(crate) shutdown: mpsc::Sender<Shutdown>,
    pub(crate) memory: MemoryBudget,
    pub(crate) large: LargeGrant,
    pub(crate) failure: Arc<crate::failure_signal::FailureSignal>,
}
impl HostSession {
    /// Identified child PID for diagnostics/tests; no PID-wide kill is used.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }
    /// Host lifetime, never reused within this parent.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Rendezvous directory removed on Close, exit or parent death.
    #[must_use]
    pub fn directory(&self) -> &std::path::Path {
        &self.directory
    }
    /// Active platform cap/monitoring policy.
    #[must_use]
    pub fn containment(&self) -> &'static str {
        self.policy
    }
    /// Subscribe to idle/busy host failure without polling a native connection.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<HostState> {
        self.state.clone()
    }
    /// Observe independent control heartbeats while data admission is withheld.
    #[must_use]
    pub fn heartbeats(&self) -> watch::Receiver<tokio::time::Instant> {
        self.heartbeat.clone()
    }
    /// Send raw control; large Grant/Resume acquires the aggregate reservation.
    /// # Errors
    /// Poisoned host, protocol saturation or memory admission failure.
    pub fn send(&mut self, request: u64, control: &Control) -> Result<(), HostFailure> {
        self.ensure_ready()?;
        self.send_ready(request, control)
            .inspect_err(|failure| self.poison(*failure))
    }
    fn send_ready(&mut self, request: u64, control: &Control) -> Result<(), HostFailure> {
        match control {
            Control::Grant(grant) | Control::Resume(grant) if grant.exclusive => {
                self.reserve_large(*grant)?;
            }
            Control::Consumed(key) => {
                let mut large = self.large.lock().map_err(|_| HostFailure::Protocol)?;
                if large.as_ref().is_some_and(|(current, _)| current == key) {
                    large.take();
                }
            }
            Control::Cancel { target } => {
                self.shutdown
                    .try_send(Shutdown::Drain(*target))
                    .map_err(|_| HostFailure::Protocol)?;
            }
            Control::Stop(key) => {
                self.shutdown
                    .try_send(Shutdown::Drain(key.request))
                    .map_err(|_| HostFailure::Protocol)?;
            }
            _ => {}
        }
        let frame = Frame::control(self.generation, request, control).map_err(HostFailure::from)?;
        self.outgoing.send(frame).map_err(HostFailure::from)
    }
    fn reserve_large(&mut self, grant: Grant) -> Result<(), HostFailure> {
        let mut large = self.large.lock().map_err(|_| HostFailure::Protocol)?;
        if let Some((key, _)) = large.as_ref() {
            if *key != grant.key {
                return Err(HostFailure::Protocol);
            }
        } else {
            *large = Some((
                grant.key,
                self.memory
                    .large((grant.encoded as usize).max(grant.decoded as usize))?,
            ));
        }
        Ok(())
    }
    /// Next control message; health is handled independently by the monitor.
    /// # Errors
    /// Host failure or ended response queue.
    pub async fn recv(&mut self) -> Result<Frame, HostFailure> {
        self.ensure_ready()?;
        tokio::select! { biased;
            changed = self.state.changed() => { let _ = changed; self.ensure_ready()?; Err(HostFailure::Exited) },
            frame = self.incoming.recv() => {
                self.ensure_ready()?;
                frame.map(|queued| queued.frame).ok_or(HostFailure::Exited)
            },
        }
    }
    /// Read one fragment only when the adapter/sink has granted admission.
    /// # Errors
    /// Invalid/stale frame or host failure.
    pub async fn read_data(&mut self) -> Result<Frame, HostFailure> {
        self.ensure_ready()?;
        let result = tokio::select! { biased;
            changed = self.state.changed() => { let _ = changed; self.ensure_ready()?; return Err(HostFailure::Exited); },
            frame = read_frame(&mut self.data, Lane::Data) => frame.map_err(transport_failure),
        };
        let result = result.and_then(|frame| {
            if frame.header.generation != self.generation {
                return Err(HostFailure::Protocol);
            }
            Ok(frame)
        });
        result.inspect_err(|error| self.poison(*error))
    }
    /// Poison a malformed Arrow/barrier/capability result from the future adapter.
    pub fn poison(&self, failure: HostFailure) {
        self.failure.fail(failure);
    }
    /// Ask Close independently of data/worker progress, then await reap.
    /// # Errors
    /// Shutdown grace expired or child failed.
    pub async fn close(mut self) -> Result<(), HostFailure> {
        self.send(0, &Control::Close)?;
        let _ = self.shutdown.send(Shutdown::Close).await;
        loop {
            let state = *self.state.borrow_and_update();
            match state {
                HostState::Closed => return Ok(()),
                HostState::Failed(error) => return Err(error),
                HostState::Ready => {}
            }
            self.state
                .changed()
                .await
                .map_err(|_| HostFailure::Exited)?;
        }
    }
    fn ensure_ready(&self) -> Result<(), HostFailure> {
        if let Some(failure) = self.failure.current()? {
            return Err(failure);
        }
        match *self.state.borrow() {
            HostState::Ready => Ok(()),
            HostState::Closed => Err(HostFailure::Exited),
            HostState::Failed(error) => Err(error),
        }
    }
}
impl Drop for HostSession {
    fn drop(&mut self) {
        self.poison(HostFailure::Exited);
    }
}

// A partial frame at transport EOF is host loss; malformed Arrow remains Protocol.
pub(crate) fn transport_failure(error: openrows_host_protocol::ProtocolError) -> HostFailure {
    match error {
        openrows_host_protocol::ProtocolError::Truncated => HostFailure::Exited,
        error => error.into(),
    }
}
