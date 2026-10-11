// SPDX-License-Identifier: Apache-2.0
//! Bounded replication lag summaries; absence is unknown, never zero.
use crate::{Reading, SampleState};
pub(crate) fn replication(reading: &Reading) -> Vec<(String, Option<f64>, SampleState)> {
    ["write_lag_ms", "flush_lag_ms", "replay_lag_ms"]
        .into_iter()
        .map(|name| {
            let index = reading
                .columns
                .iter()
                .position(|column| column.name == name);
            let value = reading
                .rows
                .iter()
                .filter_map(|row| {
                    row.get(index?)
                        .and_then(Option::as_deref)?
                        .parse::<f64>()
                        .ok()
                })
                .filter(|value| value.is_finite())
                .max_by(f64::total_cmp);
            let state = if reading.state != SampleState::Ok {
                reading.state.clone()
            } else if value.is_some() {
                SampleState::Ok
            } else {
                SampleState::Unknown
            };
            (format!("Replication:{name}"), value, state)
        })
        .collect()
}
