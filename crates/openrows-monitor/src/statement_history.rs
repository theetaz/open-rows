// SPDX-License-Identifier: Apache-2.0
//! Bounded per-query intervals share the live store's ten-minute and byte caps.
use crate::{Investigation, LiveStore, Reading, SampleState, Source, StatementInterval};
use std::collections::{BTreeMap, VecDeque};

/// Bounded real statement intervals and current monotonic identity baselines.
#[derive(Default)]
pub(crate) struct History {
    /// Ten-minute covered intervals keyed by native statement identity.
    pub(crate) intervals: BTreeMap<String, VecDeque<StatementInterval>>,
    /// Latest monotonic observation time for each currently visible identity.
    pub(crate) elapsed: BTreeMap<String, u64>,
    /// Whether retention eviction limits the history's coverage.
    pub(crate) partial: bool,
}
impl History {
    /// Reconstruct deltas from actual intervals without fabricating missing baselines.
    pub(crate) fn record(&mut self, reading: &Reading, elapsed: u64, wall: f64) {
        if reading.state != SampleState::Ok {
            *self = Self::default();
            return;
        }
        let previous = std::mem::take(&mut self.elapsed);
        for row in reading.rows.iter().take(200) {
            let cell = |name| crate::advisor::catalog::cell(reading, row, name);
            let Some(id) = cell("statement_id") else {
                continue;
            };
            let duration = previous
                .get(id)
                .map_or(0.0, |then| milliseconds(elapsed.saturating_sub(*then)));
            let state = match (reading.partial, cell("interval_state")) {
                (false, Some("ok")) => SampleState::Ok,
                (_, Some("counterReset")) => SampleState::CounterReset,
                (_, Some("gap")) => SampleState::Gap,
                _ => SampleState::Unknown,
            };
            let delta = |name| {
                cell(name)
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0 && state == SampleState::Ok)
                    .map(|v| v * duration / 1_000.0)
            };
            let interval = StatementInterval {
                statement_id: id.into(),
                collected_ms: wall,
                duration_ms: duration,
                state: state.clone(),
                calls: delta("calls"),
                total_ms: delta("total_ms"),
                rows: delta("rows"),
                shared_hit: delta("shared_blks_hit"),
                shared_read: delta("shared_blks_read"),
                temp_written: delta("temp_blks_written"),
                temp_read: delta("temp_blks_read"),
            };
            self.elapsed.insert(id.into(), elapsed);
            let points = self.intervals.entry(id.into()).or_default();
            points.push_back(interval);
            while points.len() > 41 {
                points.pop_front();
            }
        }
        self.enforce(wall);
    }
    /// Enforce the time, identity and conservative retained-byte limits.
    pub(crate) fn enforce(&mut self, wall: f64) {
        for points in self.intervals.values_mut() {
            while points
                .front()
                .is_some_and(|p| wall - p.collected_ms > 600_000.0)
            {
                points.pop_front();
            }
        }
        self.intervals.retain(|_, points| !points.is_empty());
        while self.bytes() > 512 * 1_024 || self.intervals.len() > 200 {
            let oldest = self
                .intervals
                .iter()
                .min_by(|a, b| {
                    a.1.back()
                        .map_or(0.0, |p| p.collected_ms)
                        .total_cmp(&b.1.back().map_or(0.0, |p| p.collected_ms))
                })
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                self.partial = true;
                self.intervals.remove(&key);
            } else {
                break;
            }
        }
    }
    /// Charge interval capacities and owned identities to the live-store budget.
    pub(crate) fn bytes(&self) -> usize {
        self.intervals
            .iter()
            .map(|(key, points)| {
                key.capacity()
                    + points.capacity() * 256
                    + points
                        .iter()
                        .map(|p| p.statement_id.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>()
            + self
                .elapsed
                .keys()
                .map(|key| key.capacity() + 128)
                .sum::<usize>()
    }
}

impl LiveStore {
    /// Investigate retained live intervals; `PostgreSQL` has no source percentiles.
    #[must_use]
    pub fn investigate(&self, id: &str) -> Investigation {
        let intervals: Vec<_> = self
            .statement_history
            .intervals
            .get(id)
            .map(|points| points.iter().cloned().collect())
            .unwrap_or_default();
        let valid: Vec<_> = intervals
            .iter()
            .filter(|p| p.state == SampleState::Ok)
            .collect();
        let sum = |value: fn(&StatementInterval) -> Option<f64>| {
            if valid.is_empty() {
                return None;
            }
            valid
                .iter()
                .map(|point| value(point).filter(|v| v.is_finite() && *v >= 0.0))
                .collect::<Option<Vec<_>>>()
                .map(|values| values.iter().sum::<f64>())
                .filter(|value| value.is_finite())
        };
        let calls = sum(|p| p.calls);
        let total_ms = sum(|p| p.total_ms);
        let source = self.tables.get(&Source::Statements);
        let row = source.and_then(|r| {
            r.rows
                .iter()
                .find(|row| crate::advisor::catalog::cell(r, row, "statement_id") == Some(id))
        });
        let cell = |name| {
            source
                .zip(row)
                .and_then(|(r, row)| crate::advisor::catalog::cell(r, row, name))
        };
        Investigation {
            statement_id: id.into(),
            query: cell("query").map(str::to_owned),
            calls,
            total_ms,
            mean_ms: total_ms
                .zip(calls)
                .and_then(|(total, calls)| (calls > 0.0).then_some(total / calls)),
            max_since_reset_ms: cell("max_since_reset_ms")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.0),
            regressions: crate::investigation::regressions(&intervals),
            partial: self.statement_history.partial
                || source.is_none_or(|r| r.partial)
                || valid.len() != intervals.len()
                || calls.is_none()
                || total_ms.is_none(),
            intervals,
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "bounded monotonic interval milliseconds"
)]
fn milliseconds(value: u64) -> f64 {
    value as f64
}

impl LiveStore {
    /// Project summed covered live-window totals and weighted means for ranking.
    pub(crate) fn statement_rankings(&self, reading: &Reading) -> Reading {
        let mut projected = reading.clone();
        for row in &mut projected.rows {
            let id = row.first().and_then(Option::as_deref).unwrap_or("");
            let detail = self.investigate(id);
            row.extend(
                [detail.calls, detail.total_ms, detail.mean_ms]
                    .map(|value| value.map(|number| number.to_string())),
            );
        }
        for (name, unit) in [
            ("live_window_calls", "count"),
            ("live_window_total_ms", "ms"),
            ("live_window_mean_ms", "ms"),
        ] {
            projected.columns.push(crate::Column {
                name: name.into(),
                unit: unit.into(),
                kind: crate::MetricKind::Gauge,
            });
        }
        projected
    }
}
