// SPDX-License-Identifier: Apache-2.0
//! Opt-in local monitor history write/read timings over a full day at default cadence,
//! one write per 5 s tick as the collector does it; no CI assertions.
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
// A day of ticks plus one more hour, so the last hour writes with every tier full.
const DAY_TICKS: usize = 17_280;
const TICKS: i64 = 18_000;
const BASE: i64 = 1_000 * 86_400_000;

#[test]
fn monitor_history_full_day_default_cadence_opt_in() -> TestResult {
    if std::env::var_os("OPENROWS_BENCH").is_none() {
        return Ok(());
    }
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("bench.sqlite"))?;
    store.set_monitor_profiles(&["bench".into()], BASE)?;
    let mut writes = Vec::with_capacity(usize::try_from(TICKS)?);
    for tick in 1..=TICKS {
        let now = BASE + tick * TICK_MS;
        let points: Vec<_> = METRICS.iter().map(|metric| point(metric, now)).collect();
        let began = Instant::now();
        store.record_monitor(("bench", "db"), &points, now)?;
        writes.push(began.elapsed().as_secs_f64() * 1_000.0);
    }
    let now = BASE + TICKS * TICK_MS;
    let (first_day, steady) = writes.split_at_mut(DAY_TICKS);
    report("write 16 points, first day", first_day);
    report("write 16 points, full tiers", steady);
    for hours in [1, 2, 6, 12, 24] {
        let mut reads: Vec<_> = (0..20)
            .map(|_| read_ms(&store, now - hours * 3_600_000, now))
            .collect::<Result<_, _>>()?;
        report(&format!("read {hours} h"), &mut reads);
    }
    drop(store);
    let conn = rusqlite::Connection::open(dir.path().join("bench.sqlite"))?;
    let (rows, oldest): (i64, i64) = conn.query_row(
        "SELECT count(*), min(collected_ms) FROM monitor_history",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    conn.execute_batch("VACUUM")?;
    let bytes: i64 = conn.query_row(
        "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
        [],
        |row| row.get(0),
    )?;
    report_retained(rows, (now - oldest) / 60_000, bytes);
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
fn report_retained(rows: i64, minutes: i64, bytes: i64) {
    eprintln!(
        "monitor history retained after {TICKS} ticks: {rows} rows spanning {minutes} min, \
         2 KiB charge {} KiB, whole vacuumed store {} KiB",
        rows * 2,
        bytes / 1_024
    );
}
