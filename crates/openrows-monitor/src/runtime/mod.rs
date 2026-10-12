// SPDX-License-Identifier: Apache-2.0
//! One actor owns one read-only session, with immediate stop/clear semantics.
mod actor;
mod investigation;
use crate::{Descriptor, LiveStore, MonitorError, Reading, Source, SourceStatus};
pub use investigation::DiagnosticGuard;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Boxed engine read without a Tauri or driver dependency.
pub type ReadFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Fixed bounded statistics reads on a dedicated session.
pub trait MonitorReader: Send {
    /// Probe once/open, at most sixteen small reads; never enabling instrumentation.
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>>;
    /// One guarded data query, with no cross-tick transaction.
    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading>;
    /// Out-of-band interrupt, captured before the actor starts.
    fn cancellation(&self) -> ReadFuture<'static, ()>;
}

pub(crate) struct Shared {
    pub(crate) store: LiveStore,
    pub(crate) stopped: bool,
    pub(crate) started: tokio::time::Instant,
}

/// Shared collector handle; cloning a view never creates another session.
pub struct Monitor {
    generation: u64,
    live_ms: u32,
    shared: Arc<Mutex<Shared>>,
    descriptors: Vec<Descriptor>,
    task: tokio::task::JoinHandle<()>,
    cancellation: Mutex<Option<ReadFuture<'static, ()>>>,
    events: tokio::sync::broadcast::Sender<Vec<SourceStatus>>,
    stopped: tokio::sync::watch::Sender<bool>,
    diagnostic: std::sync::atomic::AtomicBool,
    consent: Mutex<Option<(String, String)>>,
    consent_sequence: std::sync::atomic::AtomicU64,
}

impl Monitor {
    /// Spawn the sole collector actor with accepted live cadence.
    /// # Errors
    /// Unsupported interval; the passed session is dropped.
    pub fn spawn(
        reader: Box<dyn MonitorReader>,
        descriptors: Vec<Descriptor>,
        live_ms: u32,
    ) -> Result<Self, MonitorError> {
        if ![2_000, 5_000, 10_000, 30_000, 60_000].contains(&live_ms) {
            return Err(MonitorError::Unsupported);
        }
        let shared = Arc::new(Mutex::new(Shared {
            store: LiveStore::default(),
            stopped: false,
            started: tokio::time::Instant::now(),
        }));
        let (events, _) = tokio::sync::broadcast::channel(4);
        let cancellation = Mutex::new(Some(reader.cancellation()));
        let (stopped, _) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(actor::run(
            reader,
            descriptors.clone(),
            actor::Options {
                shared: shared.clone(),
                events: events.clone(),
                live_ms,
            },
        ));
        Ok(Self {
            live_ms,
            stopped,
            diagnostic: std::sync::atomic::AtomicBool::new(false),
            consent: Mutex::new(None),
            consent_sequence: std::sync::atomic::AtomicU64::new(0),
            generation: GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            shared,
            descriptors,
            task,
            cancellation,
            events,
        })
    }

    /// Clear sensitive samples synchronously and abort/drop the owning session.
    pub fn stop(&self) {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared.stopped = true;
        self.stopped.send_replace(true);
        *self.consent.lock().unwrap_or_else(PoisonError::into_inner) = None;
        shared.store = LiveStore::default();
        self.task.abort();
        if let Some(cancel) = self
            .cancellation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            tokio::spawn(async {
                let _ = tokio::time::timeout(Duration::from_secs(1), cancel).await;
            });
        }
    }

    /// Monotonic identity rejects delayed replies even if allocations are reused.
    #[must_use]
    pub fn generation(&self) -> String {
        self.generation.to_string()
    }
    /// Whether a fresh start can reuse this generation and cadence.
    #[must_use]
    pub fn reusable(&self, live_ms: u32) -> bool {
        !self.task.is_finished() && self.live_ms == live_ms
    }
    /// Redacted activity for the existing Processes view; no additional server read.
    /// # Errors
    /// No permitted current sample, or the generation is stopped.
    pub fn activity(&self) -> Result<Reading, MonitorError> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        expire(&mut shared);
        shared
            .store
            .tables
            .get(&Source::Activity)
            .filter(|reading| reading.state == crate::SampleState::Ok)
            .cloned()
            .ok_or(MonitorError::Stopped)
    }

    /// Capability requirements and reviewed guidance, never raw errors.
    #[must_use]
    pub fn capabilities(&self) -> Vec<Descriptor> {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        self.descriptors
            .iter()
            .cloned()
            .map(|mut descriptor| {
                if let Some(status) = shared.store.status.get(&descriptor.source) {
                    descriptor.state = status.state.clone();
                    descriptor.interval_ms = status.interval_ms;
                }
                descriptor
            })
            .collect()
    }
    /// Status events coalesced at most once a second.
    #[must_use]
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Vec<SourceStatus>> {
        self.events.subscribe()
    }
    /// Small source-status snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SourceStatus> {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .store
            .snapshot()
    }
    /// Latest redacted scalar aggregates; source tables and statement text are excluded.
    #[must_use]
    pub fn aggregates(&self) -> Vec<(String, f64, Option<f64>, String)> {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared
            .store
            .series
            .iter()
            .filter_map(|(key, points)| {
                let point = points.back()?;
                if !key.starts_with("Database:")
                    && !key.starts_with("sessions:")
                    && !key.starts_with("Replication:")
                {
                    return None;
                }
                let metric = if key.starts_with("Database:") {
                    format!("Database:{}", key.rsplit(':').next()?)
                } else {
                    key.clone()
                };
                Some((
                    metric,
                    point.wall_ms,
                    point.value,
                    crate::arrow::state_name(&point.state).into(),
                ))
            })
            .collect()
    }

    /// Live scalar series or source/top-N table as bounded Arrow bytes.
    /// # Errors
    /// Stopped collector or window encoding/admission failure.
    pub fn window(
        &self,
        source: Option<Source>,
        limit: u32,
        mean: bool,
    ) -> Result<Vec<u8>, MonitorError> {
        self.windowed(crate::WindowOptions {
            source,
            row_start: 0,
            row_count: limit,
            mean,
        })
    }
    /// Read a bounded page without copying an entire live window.
    /// # Errors
    /// Stopped generation or encoding failure.
    pub fn windowed(&self, options: crate::WindowOptions) -> Result<Vec<u8>, MonitorError> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        expire(&mut shared);
        shared.store.windowed(options)
    }
}
fn expire(shared: &mut Shared) {
    let elapsed = u64::try_from(shared.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    shared.store.enforce(elapsed);
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop();
    }
}
