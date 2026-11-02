// SPDX-License-Identifier: Apache-2.0
//! Downsampling capacity, aggregate exactness, unknown-name filtering and atomicity.
use super::super::monitor_rollup::{TIERS, raw_cutoff};
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const TICK_MS: i64 = 5_000;
const DAY_TICKS: i64 = DAY_MS / TICK_MS;
// Epoch-aligned to every tier width so the oldest sample opens a whole bucket.
const BASE: i64 = 1_000 * DAY_MS;
const METRICS: [&str; 16] = [
    "sessions:active",
    "sessions:idle",
    "sessions:idle in transaction",
    "sessions:waiting",
    "Database:backends",
    "Database:xact_commit",
    "Database:xact_rollback",
    "Database:blks_hit",
    "Database:blks_read",
    "Database:rows_read",
    "Database:rows_inserted",
    "Database:rows_updated",
    "Database:rows_deleted",
    "Database:temp_files",
    "Database:deadlocks",
    "Database:cache_hit_ratio",
];

/// Deterministic sample: integer values (exact sums) with a gap every 97 ticks.
fn sample(metric: &str, tick: i64) -> MonitorAggregate {
    let gap = tick % 97 == 0;
    #[expect(clippy::cast_precision_loss, reason = "small synthetic integers")]
    let value = ((tick * 37) % 101) as f64;
    MonitorAggregate {
        metric: metric.into(),
        collected_ms: BASE + tick * TICK_MS,
        value: (!gap).then_some(value),
        state: if gap { "gap" } else { "ok" }.into(),
    }
}

fn opened() -> Result<(tempfile::TempDir, LocalStore), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
    store.set_monitor_profiles(&["a".into()], BASE)?;
    Ok((dir, store))
}

/// Write `ticks` of `metrics` at default cadence, `per_call` ticks per batch of ≤ 64 points.
fn write_day(store: &LocalStore, metrics: &[&str], per_call: i64) -> TestResult {
    for first in (1..=DAY_TICKS).step_by(usize::try_from(per_call)?) {
        let last = (first + per_call - 1).min(DAY_TICKS);
        let points: Vec<_> = (first..=last)
            .flat_map(|tick| metrics.iter().map(move |metric| sample(metric, tick)))
            .collect();
        store.record_monitor(("a", "db"), &points, BASE + last * TICK_MS)?;
    }
    Ok(())
}

#[test]
fn a_day_at_default_cadence_fits_the_connection_cap_without_losing_samples() -> TestResult {
    let (_dir, store) = opened()?;
    write_day(&store, &METRICS, 4)?;
    let conn = store.lock()?;
    let (rows, oldest, valid, spans): (i64, i64, i64, i64) = conn.query_row(
        "SELECT count(*), min(collected_ms), sum(ok_count), count(DISTINCT span_ms) FROM monitor_history WHERE profile_id = 'a'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let gaps = DAY_TICKS / 97;
    // Per metric: 10 min raw (≤ 124) + 110 min of 20 s (≤ 334) + 10 h of 2 min (≤ 302)
    // + 12 h of 4 min (≤ 181) rows, against 1,600 rows per metric at the 50 MiB charge.
    assert!(rows <= 16 * 941, "{rows} rows exceed the tier arithmetic");
    assert!(
        rows * 5 < CONNECTION_ROWS * 3,
        "{rows} rows leave under 40% headroom"
    );
    assert_eq!(oldest, BASE, "the whole day is retained");
    assert_eq!(
        valid,
        (DAY_TICKS - gaps) * 16,
        "every valid sample is counted once"
    );
    assert_eq!(spans, 4);
    Ok(())
}

/// Expected read buckets: samples grouped by stored bucket start (`width`), in order.
fn expected(metric: &str, from: i64, to: i64, width: i64) -> Vec<MonitorBucket> {
    let mut groups: Vec<(i64, Vec<MonitorAggregate>)> = Vec::new();
    for tick in 1..=DAY_TICKS {
        let point = sample(metric, tick);
        let start = point.collected_ms.div_euclid(width) * width;
        if start < from || start >= to {
            continue;
        }
        match groups.last_mut() {
            Some((last, members)) if *last == start => members.push(point),
            _ => groups.push((start, vec![point])),
        }
    }
    groups
        .into_iter()
        .map(|(_, members)| summary(metric, &members))
        .collect()
}

fn summary(metric: &str, members: &[MonitorAggregate]) -> MonitorBucket {
    let values: Vec<f64> = members.iter().filter_map(|point| point.value).collect();
    let missing = members.iter().rev().find(|point| point.state != "ok");
    #[expect(clippy::cast_precision_loss, reason = "small sample counts")]
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let ok = missing.is_none();
    MonitorBucket {
        metric: metric.into(),
        collected_ms: 0,
        value: values.last().copied().filter(|_| ok),
        state: missing.map_or("ok", |point| point.state.as_str()).into(),
        min: values.iter().copied().reduce(f64::min).filter(|_| ok),
        max: values.iter().copied().reduce(f64::max).filter(|_| ok),
        mean: Some(mean).filter(|_| ok),
    }
}

fn read(store: &LocalStore, from: i64, to: i64) -> Result<Vec<MonitorBucket>, StoreError> {
    let buckets = store.monitor_history(("a", "db"), (from, to - 1))?;
    Ok(buckets
        .into_iter()
        .map(|bucket| MonitorBucket {
            collected_ms: 0,
            ..bucket
        })
        .collect())
}

#[test]
fn reads_return_exact_aggregates_over_raw_and_every_rolled_tier() -> TestResult {
    let (_dir, store) = opened()?;
    let metric = "Database:xact_commit";
    write_day(&store, &[metric], 64)?;
    let now = BASE + DAY_TICKS * TICK_MS;
    let cutoff = |index: usize| {
        let (age, width) = TIERS[index];
        (now - age).div_euclid(width) * width
    };
    let raw = raw_cutoff(now);
    // Each read bucket is no wider than the stored width, so it holds one stored row.
    let ranges = [
        (raw, now + 1, 1),
        (raw - 5_000_000, raw, TIERS[0].1),
        (cutoff(1) - 8 * 3_600_000, cutoff(1), TIERS[1].1),
        (BASE, cutoff(2), TIERS[2].1),
    ];
    for (from, to, width) in ranges {
        let want = expected(metric, from, to, width);
        assert!(want.len() > 40, "range {from}..{to} is populated");
        assert!(want.iter().any(|bucket| bucket.state == "gap"));
        assert_eq!(read(&store, from, to)?, want, "width {width}");
    }
    Ok(())
}

#[test]
fn unknown_metrics_are_skipped_and_the_rest_of_the_tick_is_kept() -> TestResult {
    let (_dir, store) = opened()?;
    let now = BASE + TICK_MS;
    let points = [
        sample("sessions:active", 1),
        sample("Database:engine_internal_text", 1),
        sample("Database:xact_commit", 1),
    ];
    assert_eq!(store.record_monitor(("a", "db"), &points, now)?, 1);
    let stored = store.monitor_history(("a", "db"), (BASE, now))?;
    let names: Vec<_> = stored.iter().map(|bucket| bucket.metric.as_str()).collect();
    assert_eq!(names, ["Database:xact_commit", "sessions:active"]);
    Ok(())
}

#[test]
fn a_failed_rollup_rolls_back_the_whole_write() -> TestResult {
    let (_dir, store) = opened()?;
    let metric = "sessions:active";
    let first: Vec<_> = (1..=12).map(|tick| sample(metric, tick)).collect();
    store.record_monitor(("a", "db"), &first, BASE + 12 * TICK_MS)?;
    store.lock()?.execute_batch(
        "CREATE TEMP TRIGGER fail_rollup BEFORE DELETE ON monitor_history WHEN old.span_ms = 0 \
         BEGIN SELECT RAISE(ABORT, 'injected'); END",
    )?;
    let late = BASE + 3_600_000;
    let failed = store.record_monitor(("a", "db"), &[sample(metric, 720)], late);
    assert!(failed.is_err(), "the injected failure surfaces");
    let rows: (i64, i64) = store.lock()?.query_row(
        "SELECT count(*), max(span_ms) FROM monitor_history",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(rows, (12, 0), "no rollup row or new sample was committed");
    Ok(())
}
