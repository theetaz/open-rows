// SPDX-License-Identifier: Apache-2.0
//! Bounded live scalar series and replaceable redacted source tables.
use crate::{MetricKind, MonitorError, Reading, SampleState, Source, SourceStatus};
use std::collections::{BTreeMap, VecDeque};

const LIVE_MS: u64 = 600_000;
const RETAINED_BYTES: usize = 2 * 1_024 * 1_024;
pub(crate) type Baseline = (String, String, u64);

#[derive(Clone)]
pub(crate) struct Point {
    pub(crate) wall_ms: f64,
    pub(crate) value: Option<f64>,
    pub(crate) state: SampleState,
    pub(crate) elapsed: u64,
    pub(crate) unit: String,
}

/// In-memory history only; deliberately has no persistence path.
#[derive(Default)]
pub struct LiveStore {
    pub(crate) tables: BTreeMap<Source, Reading>,
    pub(crate) series: BTreeMap<String, VecDeque<Point>>,
    pub(crate) status: BTreeMap<Source, SourceStatus>,
    pub(crate) baselines: BTreeMap<String, Baseline>,
    table_times: BTreeMap<Source, u64>,
}

impl LiveStore {
    /// Admit one bounded read, normalising raw SQL before any retention.
    pub fn ingest(&mut self, source: Source, mut reading: Reading, elapsed: u64, wall: f64) {
        sanitize(&mut reading);
        if source == Source::Activity && !reading.columns.is_empty() {
            crate::blockers::enrich(&mut reading);
        }
        let mut scalars = self.analyze(source, &mut reading, elapsed);
        if source == Source::Activity && reading.state == SampleState::Ok {
            scalars.extend(activity(&reading, &self.series));
        }
        for (name, value, state) in scalars {
            let unit = scalar_unit(&reading, &name).unwrap_or_else(|| {
                self.series
                    .get(&name)
                    .and_then(|points| points.back())
                    .map_or_else(|| "unknown".into(), |point| point.unit.clone())
            });
            reading.partial |= !self.push(
                name,
                Point {
                    wall_ms: wall,
                    value,
                    state,
                    elapsed,
                    unit,
                },
            );
        }
        if let Some(status) = self.status.get_mut(&source) {
            status.partial |= reading.partial;
        }
        self.tables.insert(source, reading);
        self.table_times.insert(source, elapsed);
        self.enforce(elapsed);
    }

    /// Small status snapshots contain no source rows or query text.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SourceStatus> {
        self.status.values().cloned().collect()
    }

    /// Retained allocated payload estimate, including strings and container overhead.
    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        self.tables.values().map(reading_bytes).sum::<usize>()
            + self
                .series
                .iter()
                .map(|(key, points)| key.capacity() + points.capacity() * 256)
                .sum::<usize>()
            + self
                .baselines
                .iter()
                .map(|(key, (value, epoch, _))| {
                    key.capacity() + value.capacity() + epoch.capacity() + 128
                })
                .sum::<usize>()
            + self.status.len() * 512
            + self.table_times.len() * 128
    }

    /// Encode a bounded source table/top statements or scalar live window as Arrow.
    /// # Errors
    /// Encoding/cap errors; absent tables produce a typed stopped error.
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
    /// Read a bounded, paginated Arrow window.
    /// # Errors
    /// Stopped/missing source or encoding limit.
    pub fn windowed(&self, options: crate::WindowOptions) -> Result<Vec<u8>, MonitorError> {
        crate::arrow::window(self, options)
    }

    fn push(&mut self, key: String, point: Point) -> bool {
        if self.series.len() >= 64 && !self.series.contains_key(&key) {
            return false;
        }
        let points = self.series.entry(key).or_default();
        points.push_back(point);
        while points.len() > 300 {
            points.pop_front();
        }
        true
    }

    pub(crate) fn enforce(&mut self, now: u64) {
        self.table_times.retain(|source, then| {
            let fresh = now.saturating_sub(*then) <= LIVE_MS;
            if !fresh {
                self.tables.remove(source);
            }
            fresh
        });
        self.baselines
            .retain(|_, (_, _, then)| now.saturating_sub(*then) <= LIVE_MS);
        for points in self.series.values_mut() {
            while points
                .front()
                .is_some_and(|point| now.saturating_sub(point.elapsed) > LIVE_MS)
            {
                points.pop_front();
            }
        }
        self.series.retain(|_, points| !points.is_empty());
        while self.retained_bytes() > RETAINED_BYTES {
            if let Some(key) = self
                .series
                .iter()
                .min_by_key(|(_, points)| points.front().map(|point| point.elapsed))
                .map(|(key, _)| key.clone())
            {
                self.series.remove(&key);
            } else if !self.evict_table() {
                break;
            }
            for status in self.status.values_mut() {
                status.partial = true;
            }
        }
    }

    fn evict_table(&mut self) -> bool {
        let Some(source) = self
            .tables
            .keys()
            .min_by(|a, b| {
                let time = |source| {
                    self.status
                        .get(source)
                        .map_or(0.0, |status| status.collected_ms)
                };
                time(a).total_cmp(&time(b))
            })
            .copied()
        else {
            return false;
        };
        self.tables.remove(&source);
        self.table_times.remove(&source);
        self.baselines
            .retain(|key, _| !key.starts_with(&format!("{source:?}:")));
        true
    }
}

fn scalar_unit(reading: &Reading, name: &str) -> Option<String> {
    if name.starts_with("sessions:") || name.starts_with("waits:") {
        return Some("count".into());
    }
    if name == "Database:cache_hit_ratio" {
        return Some("fraction".into());
    }
    let metric = name.rsplit(':').next()?;
    reading
        .columns
        .iter()
        .find(|column| column.name == metric)
        .map(|column| {
            if column.kind == MetricKind::Counter {
                format!("{}/s", column.unit)
            } else {
                column.unit.clone()
            }
        })
}

fn sanitize(reading: &mut Reading) {
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

fn activity(
    reading: &Reading,
    previous: &BTreeMap<String, VecDeque<Point>>,
) -> Vec<(String, Option<f64>, SampleState)> {
    let mut values: BTreeMap<String, f64> = previous
        .keys()
        .filter(|key| key.starts_with("sessions:") || key.starts_with("waits:"))
        .map(|key| (key.clone(), 0.0))
        .collect();
    for row in &reading.rows {
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
