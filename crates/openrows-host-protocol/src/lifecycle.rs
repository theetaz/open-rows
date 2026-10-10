// SPDX-License-Identifier: Apache-2.0
//! Pure request/barrier and one-batch credit state machines.
use crate::{BATCH_LIMIT, FlowKey, Grant, LARGE_LIMIT, ProtocolError, QUEUE_CAPACITY};
use std::collections::BTreeMap;

/// Monotonic request and ordered data/terminal validation for one generation.
pub struct RequestTracker {
    generation: u64,
    last: u64,
    active: BTreeMap<u64, (u32, u64)>,
}
impl RequestTracker {
    /// Track one lifetime, with no automatic adoption of old replies.
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            last: 0,
            active: BTreeMap::new(),
        }
    }
    /// Reserve a new monotonically increasing request until its terminal reply.
    /// # Errors
    /// Stale ID/generation or queue capacity exceeded.
    pub fn begin(&mut self, generation: u64, request: u64) -> Result<(), ProtocolError> {
        if generation != self.generation || request <= self.last {
            return Err(ProtocolError::Stale);
        }
        if self.active.len() == QUEUE_CAPACITY {
            return Err(ProtocolError::Limit);
        }
        self.last = request;
        self.active.insert(request, (0, 0));
        Ok(())
    }
    /// Register schema-only or stopped results before any data is authorized.
    /// # Errors
    /// Unknown request or unordered result ordinal.
    pub fn result_begin(&mut self, request: u64, result: u32) -> Result<(), ProtocolError> {
        let current = self.active.get_mut(&request).ok_or(ProtocolError::Stale)?;
        if result != current.0 + 1 {
            return Err(ProtocolError::Stale);
        }
        *current = (result, 0);
        Ok(())
    }
    /// Validate result ordinals and fragment ordering without retaining result bytes.
    /// # Errors
    /// Missing request, stale generation or unordered data.
    pub fn data(&mut self, header: crate::Header) -> Result<(), ProtocolError> {
        if header.generation != self.generation {
            return Err(ProtocolError::Stale);
        }
        let current = self
            .active
            .get_mut(&header.request)
            .ok_or(ProtocolError::Stale)?;
        if header.result == current.0 + 1 && header.sequence == 1 {
            *current = (header.result, 1);
            return Ok(());
        }
        if header.result != current.0 || header.sequence != current.1 + 1 {
            return Err(ProtocolError::Stale);
        }
        current.1 = header.sequence;
        Ok(())
    }
    /// Complete once, after the proxy observes the terminal data barrier.
    /// # Errors
    /// Duplicate terminal, unknown request or mismatched last sequence.
    pub fn finish(&mut self, request: u64, barrier: (u32, u64)) -> Result<(), ProtocolError> {
        if self.active.get(&request) != Some(&barrier) {
            return Err(ProtocolError::Stale);
        }
        self.active.remove(&request);
        Ok(())
    }
}
#[derive(Default, PartialEq)]
enum BatchPhase {
    #[default]
    Waiting,
    Granted,
    Sent,
}
/// One prepared batch and at most one authorized batch, independent of I/O.
#[derive(Default)]
pub struct CreditState {
    offered: Option<Grant>,
    phase: BatchPhase,
    paused: bool,
    stopped: bool,
    consumed: Option<FlowKey>,
}
impl CreditState {
    /// Offer a checked batch, refusing source/encoded over-allocation.
    /// # Errors
    /// Outstanding batch, zero sizes or ceiling overflow.
    pub fn offer(&mut self, offer: Grant) -> Result<(), ProtocolError> {
        validate_grant(offer)?;
        if self.offered.is_some() {
            return Err(ProtocolError::Stale);
        }
        self.offered = Some(offer);
        self.phase = BatchPhase::Waiting;
        Ok(())
    }
    /// Authorize exactly the offered batch; old duplicate credit is idempotent.
    /// # Errors
    /// Wrong batch identity, insufficient credit or unsolicited large admission.
    pub fn grant(&mut self, grant: Grant, resume: bool) -> Result<(), ProtocolError> {
        if self.consumed == Some(grant.key) {
            return Ok(());
        }
        validate_grant(grant)?;
        let offer = self.offered.ok_or(ProtocolError::Stale)?;
        if offer.key != grant.key
            || grant.encoded < offer.encoded
            || grant.decoded < offer.decoded
            || offer.exclusive != grant.exclusive
        {
            return Err(ProtocolError::Stale);
        }
        if self.phase == BatchPhase::Sent {
            return Ok(());
        }
        if resume {
            self.paused = false;
        }
        if !self.paused {
            self.phase = BatchPhase::Granted;
        }
        Ok(())
    }
    /// Pause revokes future credit; the already-authorized batch may finish.
    pub fn pause(&mut self) {
        self.paused = true;
    }
    /// Stop drains the outstanding batch and prevents fetching another.
    pub fn stop(&mut self) {
        self.stopped = true;
    }
    /// True when the worker may send the offered batch.
    #[must_use]
    pub fn authorized(&self) -> bool {
        self.phase == BatchPhase::Granted && !self.stopped
    }
    /// True when rows should cease without user cancellation.
    #[must_use]
    pub fn stopped(&self) -> bool {
        self.stopped
    }
    /// Mark the one admitted batch sent, before fetching another.
    /// # Errors
    /// Sending without an exact grant.
    pub fn sent(&mut self) -> Result<(), ProtocolError> {
        if !self.authorized() {
            return Err(ProtocolError::Stale);
        }
        self.phase = BatchPhase::Sent;
        Ok(())
    }
    /// Return credit only once the caller's sink accepted the batch.
    /// # Errors
    /// Wrong batch or consumption before send.
    pub fn consume(&mut self, key: FlowKey) -> Result<(), ProtocolError> {
        if self.consumed == Some(key) {
            return Ok(());
        }
        if self.phase != BatchPhase::Sent || self.offered.map(|offer| offer.key) != Some(key) {
            return Err(ProtocolError::Stale);
        }
        self.offered = None;
        self.phase = BatchPhase::Waiting;
        self.consumed = Some(key);
        Ok(())
    }
    /// Whether the prepared batch has reached the caller's sink.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.offered.is_some()
    }
}
fn validate_grant(grant: Grant) -> Result<(), ProtocolError> {
    let maximum = if grant.exclusive {
        LARGE_LIMIT
    } else {
        BATCH_LIMIT
    };
    if grant.encoded == 0
        || grant.decoded == 0
        || grant.encoded as usize > maximum
        || grant.decoded as usize > maximum
    {
        Err(ProtocolError::Limit)
    } else {
        Ok(())
    }
}
