// SPDX-License-Identifier: Apache-2.0
//! Opt-in local monitor history write/read timings at default cadence; no CI assertions.
use openrows_store::{LocalStore, MonitorAggregate};
use std::time::Instant;

type TestResult = Result<(), Box<dyn std::error::Error>>;
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
const TICK_MS: i64 = 5_000;
// 16 points per 5 s tick reach the 25,600-row (50 MiB at 2 KiB) cap after 1,600 ticks.
const CAP_TICKS: usize = 1_600;
const TICKS: i64 = 2_000;

#[test]
fn monitor_history_default_cadence_to_cap_opt_in() -> TestResult {
    if std::env::var_os("OPENROWS_BENCH").is_none() {
        return Ok(());
    }
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("bench.sqlite"))?;
    store.set_monitor_profiles(&["bench".into()], 0)?;
    let mut writes = Vec::with_capacity(usize::try_from(TICKS)?);
    for tick in 1..=TICKS {
        let now = tick * TICK_MS;
        let points: Vec<_> = METRICS.iter().map(|metric| point(metric, now)).collect();
        let began = Instant::now();
        store.record_monitor(("bench", "db"), &points, now)?;
        writes.push(began.elapsed().as_secs_f64() * 1_000.0);
    }
    let now = TICKS * TICK_MS;
    let retained = store.monitor_history(("bench", "db"), (0, now))?;
    let (before_cap, at_cap) = writes.split_at_mut(CAP_TICKS);
    report("write 16 points below cap", before_cap);
    report("write 16 points at cap", at_cap);
    for hours in [1, 6, 24] {
        let mut reads: Vec<_> = (0..20)
            .map(|_| read_ms(&store, now - hours * 3_600_000, now))
            .collect::<Result<_, _>>()?;
        report(&format!("read {hours} h"), &mut reads);
    }
    let oldest = retained.iter().map(|p| p.collected_ms).min().unwrap_or(now);
    report_retained(retained.len(), now - oldest);
    Ok(())
}

fn point(metric: &str, now: i64) -> MonitorAggregate {
    MonitorAggregate {
        metric: metric.into(),
        collected_ms: now,
        value: Some(1.0),
        state: "ok".into(),
    }
}

fn read_ms(store: &LocalStore, since: i64, now: i64) -> Result<f64, Box<dyn std::error::Error>> {
    let began = Instant::now();
    store.monitor_history(("bench", "db"), (since, now))?;
    Ok(began.elapsed().as_secs_f64() * 1_000.0)
}

#[expect(clippy::print_stderr, reason = "opt-in benchmark timings only")]
fn report(case: &str, timings: &mut [f64]) {
    timings.sort_by(f64::total_cmp);
    let at = |q: usize| timings.get(timings.len() * q / 100).copied().unwrap_or(0.0);
    let max = timings.last().copied().unwrap_or(0.0);
    eprintln!(
        "monitor history {case}: n={} p50={:.3}ms p95={:.3}ms max={max:.3}ms",
        timings.len(),
        at(50),
        at(95)
    );
}

#[expect(clippy::print_stderr, reason = "opt-in benchmark retained coverage")]
fn report_retained(buckets: usize, span_ms: i64) {
    eprintln!(
        "monitor history retained after {TICKS} ticks: {buckets} read buckets spanning {} min",
        span_ms / 60_000
    );
}
