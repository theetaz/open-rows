// SPDX-License-Identifier: Apache-2.0
//! Bounded source sanitization and aggregate activity projections.
use crate::{MetricKind, Reading, SampleState, retention::Point};
use std::collections::{BTreeMap, VecDeque};

/// Redact query literals and enforce source shape/text/byte limits before retention.
pub(crate) fn sanitize(reading: &mut Reading) {
    reading.partial |= reading.columns.len() > 128;
    reading.columns.truncate(128);
    if reading.rows.len() >= 1_000 {
        reading.partial = true;
    }
    reading.rows.truncate(1_000);
    for row in &mut reading.rows {
        row.truncate(reading.columns.len());
        row.resize(reading.columns.len(), None);
        for (cell, column) in row.iter_mut().zip(&reading.columns) {
            if column.kind == MetricKind::Query {
                *cell = cell
                    .as_deref()
                    .and_then(openrows_sql::normalize_monitor_query);
            } else if cell.as_ref().is_some_and(|text| text.len() > 4_096) {
                *cell = None;
                reading.partial = true;
            }
        }
    }
    if reading_bytes(reading) > 2 * 1_024 * 1_024 {
        reading.rows.clear();
        reading.partial = true;
        reading.state = SampleState::Gap;
    }
}

/// Conservatively charge source capacities and owned text to the shared byte cap.
pub(crate) fn reading_bytes(reading: &Reading) -> usize {
    reading.rows.capacity() * 32
        + reading.epoch.capacity()
        + 256
        + reading
            .rows
            .iter()
            .map(|row| {
                row.capacity() * 32 + row.iter().flatten().map(String::capacity).sum::<usize>()
            })
            .sum::<usize>()
        + reading
            .columns
            .iter()
            .map(|col| col.name.capacity() + col.unit.capacity() + 128)
            .sum::<usize>()
}

/// Derive bounded current session/wait counts from a permitted activity snapshot.
pub(crate) fn activity(
    reading: &Reading,
    previous: &BTreeMap<String, VecDeque<Point>>,
) -> Vec<(String, Option<f64>, SampleState)> {
    let mut values: BTreeMap<String, f64> = previous
        .keys()
        .filter(|key| key.starts_with("sessions:") || key.starts_with("waits:"))
        .map(|key| (key.clone(), 0.0))
        .collect();
    for name in [
        "active",
        "idle",
        "idle in transaction",
        "idle in transaction (aborted)",
        "waiting",
    ] {
        values.insert(format!("sessions:{name}"), 0.0);
    }
    for row in &reading.rows {
        if row
            .get(2)
            .and_then(Option::as_deref)
            .is_some_and(|wait| !wait.is_empty())
            && row.get(1).and_then(Option::as_deref) == Some("active")
        {
            *values.entry("sessions:waiting".into()).or_default() += 1.0;
        }
        if let Some(state) = row.get(1).and_then(Option::as_deref) {
            *values.entry(format!("sessions:{state}")).or_default() += 1.0;
        }
        if let Some(wait) = row.get(2).and_then(Option::as_deref) {
            *values.entry(format!("waits:{wait}")).or_default() += 1.0;
        }
    }
    values
        .into_iter()
        .take(16)
        .map(|(key, value)| (key, Some(value), SampleState::Ok))
        .collect()
}
