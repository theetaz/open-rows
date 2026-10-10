// SPDX-License-Identifier: Apache-2.0
//! Bounded transport counters measure grants without retaining row data.
use openrows_host_protocol::Grant;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// High-water marks for one connection's transport, excluding retained store batches.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostMetrics {
    /// Maximum simultaneous authorized batches.
    pub outstanding_grants: usize,
    /// Largest admitted encoded segment.
    pub encoded_bytes: usize,
    /// Largest admitted decoded Arrow allocation including dictionaries/schema.
    pub decoded_bytes: usize,
}
#[derive(Default)]
pub(crate) struct Metrics {
    current: AtomicUsize,
    peak: AtomicUsize,
    encoded: AtomicUsize,
    decoded: AtomicUsize,
}
impl Metrics {
    pub(crate) fn snapshot(&self) -> HostMetrics {
        HostMetrics {
            outstanding_grants: self.peak.load(Ordering::SeqCst),
            encoded_bytes: self.encoded.load(Ordering::SeqCst),
            decoded_bytes: self.decoded.load(Ordering::SeqCst),
        }
    }
    pub(crate) fn admit(self: &Arc<Self>, grant: Grant) -> Window {
        let current = self.current.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(current, Ordering::SeqCst);
        self.encoded
            .fetch_max(grant.encoded as usize, Ordering::SeqCst);
        self.decoded
            .fetch_max(grant.decoded as usize, Ordering::SeqCst);
        Window(self.clone())
    }
}
pub(crate) struct Window(Arc<Metrics>);
impl Drop for Window {
    fn drop(&mut self) {
        self.0.current.fetch_sub(1, Ordering::SeqCst);
    }
}
