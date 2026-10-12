// SPDX-License-Identifier: Apache-2.0
//! Weighted interval investigation; `PostgreSQL` summaries provide no percentiles.
use crate::SampleState;
use serde::Serialize;

/// One reset-aware statement interval; deltas never include a reset baseline.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StatementInterval {
    /// Database/user/query-scoped identity.
    pub statement_id: String,
    /// Interval end wall time.
    pub collected_ms: f64,
    /// Actual monotonic duration, not configured cadence.
    pub duration_ms: f64,
    /// Validity of this interval.
    pub state: SampleState,
    /// Call count delta.
    pub calls: Option<f64>,
    /// Execution time delta in milliseconds.
    pub total_ms: Option<f64>,
    /// Returned/affected row delta; not rows examined.
    pub rows: Option<f64>,
    /// Shared buffer hits.
    pub shared_hit: Option<f64>,
    /// Shared buffer reads, not physical disk reads.
    pub shared_read: Option<f64>,
    /// Temporary block writes, not byte counts.
    pub temp_written: Option<f64>,
    /// Temporary block reads.
    pub temp_read: Option<f64>,
}
impl StatementInterval {
    /// Mean execution time from real delta totals/calls.
    #[must_use]
    pub fn mean_ms(&self) -> Option<f64> {
        self.total_ms
            .zip(self.calls)
            .and_then(|(total, calls)| {
                (self.state == SampleState::Ok
                    && calls.is_finite()
                    && calls > 0.0
                    && total.is_finite()
                    && total >= 0.0)
                    .then_some(total / calls)
            })
            .filter(|mean| mean.is_finite())
    }
}

/// A deterministic comparison between two complete adjacent intervals.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Regression {
    /// Earlier interval end.
    pub previous_ms: f64,
    /// Current interval end.
    pub current_ms: f64,
    /// Earlier mean.
    pub previous_mean_ms: f64,
    /// Current mean.
    pub current_mean_ms: f64,
}

/// Bounded detail with explicit source coverage and no fabricated distributions.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Investigation {
    /// Scoped statement identity.
    pub statement_id: String,
    /// Safely normalized SQL, or fingerprint-only.
    pub query: Option<String>,
    /// Retained live intervals; absent observations remain gaps.
    pub intervals: Vec<StatementInterval>,
    /// Window delta calls.
    pub calls: Option<f64>,
    /// Window execution time delta.
    pub total_ms: Option<f64>,
    /// Summed time / summed calls, never a mean of means.
    pub mean_ms: Option<f64>,
    /// Source maximum since reset; never subtracted or labelled a window maximum.
    pub max_since_reset_ms: Option<f64>,
    /// Adjacent complete-interval regressions.
    pub regressions: Vec<Regression>,
    /// Truncation, unknown baselines or gaps limit coverage.
    pub partial: bool,
}

/// Detect only threshold-crossing adjacent complete intervals with sufficient calls.
pub(crate) fn regressions(intervals: &[StatementInterval]) -> Vec<Regression> {
    intervals
        .windows(2)
        .filter_map(|pair| {
            let [previous, current] = pair else {
                return None;
            };
            let (before, after) = (previous.mean_ms()?, current.mean_ms()?);
            let contiguous = (current.collected_ms - current.duration_ms - previous.collected_ms)
                .abs()
                < 1_000.0;
            (contiguous
                && previous.calls? >= 5.0
                && current.calls? >= 5.0
                && after >= before * 2.0
                && after - before >= 10.0)
                .then_some(Regression {
                    previous_ms: previous.collected_ms,
                    current_ms: current.collected_ms,
                    previous_mean_ms: before,
                    current_mean_ms: after,
                })
        })
        .collect()
}
