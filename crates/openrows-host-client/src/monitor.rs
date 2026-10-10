// SPDX-License-Identifier: Apache-2.0
//! Exit, heartbeat and memory detection stays independent of data/worker progress.
use crate::{
    CLOSE_GRACE, HOST_MEMORY, HostFailure, HostReservation, HostState, platform, session::Shutdown,
};
use openrows_host_protocol::{Control, ControlQueue, Frame};
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tokio::{
    process::Child,
    sync::{mpsc, watch},
    task::JoinHandle,
    time::Instant,
};

pub(crate) struct Monitor {
    pub(crate) child: Child,
    pub(crate) failure: std::sync::Arc<crate::failure_signal::FailureSignal>,
    pub(crate) directory: TempDir,
    pub(crate) reservation: HostReservation,
    pub(crate) large: crate::session::LargeGrant,
    pub(crate) platform: platform::Guard,
    pub(crate) circuit: crate::CircuitBreaker,
    pub(crate) generation: u64,
    pub(crate) control: ControlQueue,
    pub(crate) shutdown: mpsc::Receiver<Shutdown>,
    pub(crate) heartbeat: watch::Receiver<Instant>,
    pub(crate) state: watch::Sender<HostState>,
    pub(crate) tasks: Vec<JoinHandle<()>>,
}
impl Monitor {
    pub(crate) async fn run(mut self) {
        let failure = self.observe().await.map(|reason| {
            self.failure.record(reason);
            self.failure
                .current()
                .unwrap_or(Some(HostFailure::Protocol))
                .unwrap_or(reason)
        });
        for task in &self.tasks {
            task.abort();
        }
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        if failure.is_some() {
            let _ = self.child.start_kill();
        }
        let _ = self.child.wait().await;
        drop(self.platform);
        drop(self.reservation);
        self.large
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let _ = self.directory.close();
        if let Some(failure) = failure {
            self.circuit.record_exit();
            self.state.send_replace(HostState::Failed(failure));
        } else {
            self.state.send_replace(HostState::Closed);
        }
    }
    async fn observe(&mut self) -> Option<HostFailure> {
        let mut sample = tokio::time::interval(Duration::from_millis(100));
        sample.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut health = Health::new();
        let mut drains = std::collections::BTreeMap::new();
        let mut last_completed = 0;
        loop {
            tokio::select! { biased;

                shutdown = self.shutdown.recv() => match shutdown {
                    Some(Shutdown::Close) => return self.grace().await,
                    Some(Shutdown::Failure(failure)) => return Some(failure),
                    Some(Shutdown::Drain(target)) => { if target > last_completed { drains.entry(target).or_insert_with(|| Instant::now() + CLOSE_GRACE); } },
                    Some(Shutdown::Completed(request)) => { last_completed = last_completed.max(request); drains.remove(&request); },
                    None => return Some(HostFailure::Exited),
                },
                status = self.child.wait() => { let _ = status; return Some(HostFailure::Exited); },
                _ = sample.tick() => {
                    if drains.values().any(|deadline| Instant::now() >= *deadline) { return Some(HostFailure::Unresponsive); }
                    let pid = self.child.id()?;
                    if memory_exceeds_limit(platform::memory(pid)) { return Some(HostFailure::MemoryLimit); }
                    match health.check(*self.heartbeat.borrow()) {
                        HealthAction::Alive => {},
                        HealthAction::Probe => {
                            let frame = Frame::control(self.generation, 0, &Control::Heartbeat).ok()?;
                            if self.control.send(frame).is_err() { return Some(HostFailure::Protocol); }
                        }
                        HealthAction::Failed => return Some(HostFailure::Unresponsive),
                    }
                }
            }
        }
    }
    async fn grace(&mut self) -> Option<HostFailure> {
        match tokio::time::timeout(CLOSE_GRACE, self.child.wait()).await {
            Ok(Ok(status)) if status.success() => None,
            Ok(_) => Some(HostFailure::Exited),
            Err(_) => Some(HostFailure::Unresponsive),
        }
    }
}
struct Health {
    probe: Option<Instant>,
    last_sample: Instant,
    wall: SystemTime,
    reset: Instant,
}
enum HealthAction {
    Alive,
    Probe,
    Failed,
}
impl Health {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            probe: None,
            last_sample: now,
            wall: SystemTime::now(),
            reset: now,
        }
    }
    fn check(&mut self, heartbeat: Instant) -> HealthAction {
        let now = Instant::now();
        let wall = SystemTime::now();
        let elapsed = now.duration_since(self.last_sample);
        let wall_elapsed = wall.duration_since(self.wall).unwrap_or_default();
        if elapsed > Duration::from_secs(2)
            || wall_elapsed.saturating_sub(elapsed) > Duration::from_secs(2)
        {
            self.reset = now;
            self.probe = None;
        }
        self.last_sample = now;
        self.wall = wall;
        let last = heartbeat.max(self.reset);
        if now.duration_since(last) < Duration::from_secs(3) {
            self.probe = None;
            return HealthAction::Alive;
        }
        match self.probe {
            Some(probed) if now.duration_since(probed) >= Duration::from_secs(1) => {
                HealthAction::Failed
            }
            Some(_) => HealthAction::Alive,
            None => {
                self.probe = Some(now);
                HealthAction::Probe
            }
        }
    }
}

fn memory_exceeds_limit(sample: std::io::Result<usize>) -> bool {
    sample.is_ok_and(|bytes| bytes > HOST_MEMORY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_observed_over_threshold_sample_is_a_memory_limit() {
        for error in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::InvalidData,
        ] {
            assert!(!memory_exceeds_limit(Err(error.into())));
        }
        assert!(!memory_exceeds_limit(Ok(HOST_MEMORY - 1)));
        assert!(!memory_exceeds_limit(Ok(HOST_MEMORY)));
        assert!(memory_exceeds_limit(Ok(HOST_MEMORY + 1)));
    }
}
