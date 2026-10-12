// SPDX-License-Identifier: Apache-2.0
//! Counter baselines have bounded identities and explicit per-statement validity.
use crate::{Column, LiveStore, MetricKind, Reading, SampleState, Source, retention::Baseline};
use std::collections::BTreeMap;
type Scalar = (String, Option<f64>, SampleState);
struct Context<'a> {
    source: Source,
    prefix: String,
    epoch: &'a str,
    now: u64,
    columns: &'a [Column],
    previous: &'a BTreeMap<String, Baseline>,
}

impl LiveStore {
    /// Baseline bounded identities and expose valid rates without fabricated values.
    pub(crate) fn analyze(
        &mut self,
        source: Source,
        reading: &mut Reading,
        now: u64,
    ) -> Vec<Scalar> {
        let prefix = format!("{source:?}:");
        let previous = std::mem::take(&mut self.baselines);
        self.baselines = previous
            .iter()
            .filter(|(key, _)| !key.starts_with(&prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        if reading.state != SampleState::Ok {
            return self
                .series
                .keys()
                .filter(|key| {
                    key.starts_with(&prefix)
                        || (source == Source::Activity
                            && (key.starts_with("sessions:") || key.starts_with("waits:")))
                })
                .map(|key| (key.clone(), None, reading.state.clone()))
                .collect();
        }
        let context = Context {
            source,
            prefix,
            epoch: &reading.epoch,
            now,
            columns: &reading.columns,
            previous: &previous,
        };
        let mut scalars = Vec::new();
        for row in reading.rows.iter_mut().take(200) {
            scalars.extend(analyze_row(&context, row, &mut self.baselines));
        }
        if rate_table(source) {
            append_rates(reading);
        }
        if source == Source::Statements {
            reading.rows.truncate(200);
            reading.partial |= reading.rows.len() == 200;
            rate_units(&mut reading.columns);
            reading.columns.extend([
                Column {
                    name: "interval_state".into(),
                    unit: "state".into(),
                    kind: MetricKind::Text,
                },
                Column {
                    name: "window_mean_ms".into(),
                    unit: "ms".into(),
                    kind: MetricKind::Gauge,
                },
            ]);
        }
        if source == Source::Database {
            add_ratio(&mut scalars);
        }
        scalars
    }
}

fn analyze_row(
    context: &Context<'_>,
    row: &mut Vec<Option<String>>,
    baselines: &mut BTreeMap<String, Baseline>,
) -> Vec<Scalar> {
    let identity = row
        .first()
        .and_then(Option::as_deref)
        .unwrap_or("unknown")
        .to_owned();
    let mut scalars = Vec::new();
    let mut row_state = SampleState::Ok;
    let mut rates = Vec::new();
    for (index, column) in context.columns.iter().enumerate().filter(|(_, col)| {
        col.kind == MetricKind::Counter
            || (context.source == Source::Database && col.kind == MetricKind::Gauge)
    }) {
        let (key, number, state) = analyze_cell(
            context,
            row.get(index).and_then(Option::as_deref),
            (&identity, column),
            baselines,
        );
        if context.source == Source::Database {
            scalars.push((key, number, state.clone()));
        }
        if context.source == Source::Statements && column.kind == MetricKind::Counter {
            if state != SampleState::Ok {
                row_state = state.clone();
            }
            row[index] = number.map(|value| value.to_string());
        }
        if rate_table(context.source) && column.kind == MetricKind::Counter {
            if state != SampleState::Ok {
                row_state = state;
            }
            rates.push(number.map(|value| value.to_string()));
        }
    }
    if rate_table(context.source) {
        row.extend(rates);
        row.push(Some(crate::arrow::state_name(&row_state).into()));
    }
    if context.source == Source::Statements {
        append_statement_mean(row, &row_state);
    }
    scalars
}

fn analyze_cell(
    context: &Context<'_>,
    value: Option<&str>,
    (identity, column): (&str, &Column),
    baselines: &mut BTreeMap<String, Baseline>,
) -> Scalar {
    let key = format!("{}{identity}:{}", context.prefix, column.name);
    let (number, state) = observation(
        value,
        context.previous.get(&key),
        (context.epoch, context.now),
        column.kind,
    );
    if let Some(value) = value {
        baselines.insert(
            key.clone(),
            (value.to_owned(), context.epoch.into(), context.now),
        );
    }
    (key, number, state)
}

fn append_statement_mean(row: &mut Vec<Option<String>>, state: &SampleState) {
    let number = |index: usize| {
        row.get(index)
            .and_then(Option::as_deref)
            .and_then(|value| value.parse::<f64>().ok())
    };
    let mean = number(2)
        .zip(number(1))
        .and_then(|(total, calls)| (calls > 0.0).then_some(total / calls));
    row.extend([
        Some(crate::arrow::state_name(state).into()),
        mean.map(|value| value.to_string()),
    ]);
}

fn rate_table(source: Source) -> bool {
    matches!(source, Source::Tables | Source::Indexes | Source::Io)
}

fn append_rates(reading: &mut Reading) {
    let derived: Vec<_> = reading
        .columns
        .iter()
        .filter(|column| column.kind == MetricKind::Counter)
        .map(|column| Column {
            name: format!("{}_rate", column.name),
            unit: format!("{}/s", column.unit),
            kind: MetricKind::Gauge,
        })
        .collect();
    let missing = derived.len();
    for row in reading.rows.iter_mut().skip(200) {
        row.extend(std::iter::repeat_n(None, missing));
        row.push(Some("unknown".into()));
    }
    reading.partial |= reading.rows.len() > 200;
    reading.columns.extend(derived);
    reading.columns.push(Column {
        name: "interval_state".into(),
        unit: "state".into(),
        kind: MetricKind::Text,
    });
}

fn observation(
    value: Option<&str>,
    previous: Option<&Baseline>,
    (epoch, now): (&str, u64),
    kind: MetricKind,
) -> (Option<f64>, SampleState) {
    let Some(value) = value else {
        return (None, SampleState::Unknown);
    };
    if kind == MetricKind::Gauge {
        let number = value.parse::<f64>().ok().filter(|v| v.is_finite());
        return (
            number,
            if number.is_some() {
                SampleState::Ok
            } else {
                SampleState::Unknown
            },
        );
    }
    let Some((old, old_epoch, then)) = previous else {
        return (None, SampleState::Unknown);
    };
    if epoch != old_epoch {
        return (None, SampleState::CounterReset);
    }
    let delta = match (value.parse::<u64>(), old.parse::<u64>()) {
        (Ok(value), Ok(old)) => value.checked_sub(old).map(exact_delta),
        _ => value
            .parse::<f64>()
            .ok()
            .zip(old.parse::<f64>().ok())
            .and_then(|(value, old)| (value >= old && old >= 0.0).then_some(value - old)),
    };
    let Some(delta) = delta.filter(|value| value.is_finite()) else {
        return (None, SampleState::CounterReset);
    };
    let Some(ms) = now
        .checked_sub(*then)
        .filter(|ms| *ms > 0 && *ms <= 600_000)
    else {
        return (None, SampleState::Gap);
    };
    (Some(delta * 1_000.0 / exact_delta(ms)), SampleState::Ok)
}
#[expect(
    clippy::cast_precision_loss,
    reason = "exact integer subtraction precedes rate conversion"
)]
fn exact_delta(value: u64) -> f64 {
    value as f64
}

fn add_ratio(scalars: &mut Vec<Scalar>) {
    let metric = |name: &str| {
        scalars
            .iter()
            .find(|(key, _, _)| key.ends_with(name))
            .and_then(|(_, value, _)| *value)
    };
    let has = |name: &str| scalars.iter().any(|(key, _, _)| key.ends_with(name));
    let mysql = has(":buffer_read_requests");
    // SQL Server reports its buffer cache hit ratio directly.
    if !mysql && !has(":blks_hit") {
        return;
    }
    let cache = if mysql {
        metric(":buffer_reads")
            .zip(metric(":buffer_read_requests"))
            .filter(|(reads, requests)| *requests > 0.0 && reads <= requests)
            .map(|(reads, requests)| 1.0 - reads / requests)
    } else {
        crate::ratio(metric(":blks_hit"), metric(":blks_read"))
    };
    scalars.push((
        if mysql {
            "Database:buffer_pool_hit_ratio"
        } else {
            "Database:cache_hit_ratio"
        }
        .into(),
        cache,
        if cache.is_some() {
            SampleState::Ok
        } else {
            SampleState::Unknown
        },
    ));
}

fn rate_units(columns: &mut [Column]) {
    for column in columns {
        if column.kind == MetricKind::Counter {
            column.kind = MetricKind::Gauge;
            column.unit.push_str("/s");
        }
    }
}
