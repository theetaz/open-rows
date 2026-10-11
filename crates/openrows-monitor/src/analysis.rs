// SPDX-License-Identifier: Apache-2.0
//! Reset/rollover-aware deltas using real monotonic intervals.
use crate::SampleState;

/// A scoped cumulative observation.
#[derive(Clone, Debug)]
pub struct Counter {
    /// Exact counter value retained as integer until subtraction.
    pub value: u64,
    /// Server/reset/generation identity.
    pub epoch: String,
    /// Monotonic elapsed milliseconds.
    pub elapsed_ms: u64,
}

/// A rate, or an explicit missing/reset baseline; wrap is never inferred.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "convert exact integer delta only after subtraction"
)]
pub fn counter_delta(previous: Option<&Counter>, current: &Counter) -> (Option<f64>, SampleState) {
    let Some(previous) = previous else {
        return (None, SampleState::Unknown);
    };
    if previous.epoch != current.epoch || current.value < previous.value {
        return (None, SampleState::CounterReset);
    }
    let Some(elapsed) = current
        .elapsed_ms
        .checked_sub(previous.elapsed_ms)
        .filter(|ms| *ms > 0)
    else {
        return (None, SampleState::Gap);
    };
    (
        Some((current.value - previous.value) as f64 * 1_000.0 / elapsed as f64),
        SampleState::Ok,
    )
}

/// A cache ratio with a positive denominator; no observations remain unknown.
#[must_use]
pub fn ratio(hits: Option<f64>, reads: Option<f64>) -> Option<f64> {
    let (hits, reads) = (hits?, reads?);
    ((hits + reads) > 0.0).then(|| hits / (hits + reads))
}
