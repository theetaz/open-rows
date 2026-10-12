// SPDX-License-Identifier: Apache-2.0
//! Opt-in 60-second default-cadence collector evidence; no timing assertions in CI.
use super::support::Collect;
use openrows_driver_api::Session;
use openrows_driver_mysql::MySqlSession;
use openrows_monitor::{Cadence, LiveStore, MonitorReader, SampleState, Source};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[expect(
    clippy::print_stderr,
    reason = "opt-in benchmark report, never source text or values"
)]
pub(super) async fn measure(monitor: &mut MySqlSession) -> TestResult {
    let descriptors = monitor.discover().await;
    let before = process()?;
    let mut store = LiveStore::default();
    let mut schedules: Vec<_> = descriptors
        .into_iter()
        .filter(|d| d.state == SampleState::Ok)
        .map(|d| d.source)
        .map(|source| (source, Cadence::new(source.floor_ms(5_000)), 0_u32))
        .collect();
    let start = Instant::now();
    let mut durations = Vec::new();
    let mut ticks = tokio::time::interval(Duration::from_millis(100));
    while start.elapsed() < Duration::from_secs(60) {
        ticks.tick().await;
        if start.elapsed() >= Duration::from_secs(60) {
            break;
        }
        let now = u64::try_from(start.elapsed().as_millis())?;
        for (source, cadence, count) in &mut schedules {
            if !cadence.due(now) {
                continue;
            }
            let began = Instant::now();
            let reading = monitor.read(*source, cadence.deadline_ms()).await;
            let duration = began.elapsed().as_secs_f64() * 1_000.0;
            *count += 1;
            durations.push(duration);
            cadence.complete(
                u64::try_from(start.elapsed().as_millis())?,
                true,
                reading.state != SampleState::Ok,
                u32::try_from(began.elapsed().as_millis())?,
            );
            store.ingest(
                *source,
                reading,
                now,
                start.elapsed().as_secs_f64() * 1_000.0,
            );
        }
    }
    let after = process()?;
    durations.sort_by(f64::total_cmp);
    let p95 = durations
        .get(durations.len() * 95 / 100)
        .copied()
        .unwrap_or_default();
    let reads: u32 = schedules.iter().map(|(_, _, count)| count).sum();
    let per_source: Vec<_> = schedules
        .iter()
        .map(|(source, _, count)| format!("{source:?}={count}"))
        .collect();
    eprintln!(
        "mysql/mariadb monitor overhead: 60s, data reads/min={reads} ({}; 4 round trips each with guards), read+guard p95={p95:.3}ms, retained={}bytes, RSS before/after={}/{}KiB, CPU before/after={}/{}",
        per_source.join(" "),
        store.retained_bytes(),
        before.0,
        after.0,
        before.1,
        after.1
    );
    analysis(monitor).await?;
    server_durations(monitor).await
}

fn process() -> TestResult<(u64, String)> {
    let output = Command::new("ps")
        .args(["-p", &std::process::id().to_string(), "-o", "rss=,time="])
        .output()?;
    if !output.status.success() {
        return Err("ps failed".into());
    }
    let text = String::from_utf8(output.stdout)?;
    let mut values = text.split_whitespace();
    Ok((
        values.next().ok_or("rss")?.parse()?,
        values.next().ok_or("cpu time")?.into(),
    ))
}

#[expect(clippy::print_stderr, reason = "opt-in pure analysis p95 evidence")]
async fn analysis(monitor: &mut MySqlSession) -> TestResult {
    let mut sample = monitor.read(Source::Tables, 1_000).await;
    let row = sample
        .rows
        .first()
        .ok_or("relation fixture missing")?
        .clone();
    sample.rows = (0..1_000)
        .map(|index| {
            let mut row = row.clone();
            row[0] = Some(index.to_string());
            row
        })
        .collect();
    let mut durations = Vec::new();
    let mut store = LiveStore::default();
    for index in 0..100 {
        // The actor receives an owned read; fixture duplication is not sample analysis.
        let reading = sample.clone();
        let began = Instant::now();
        store.ingest(Source::Tables, reading, index * 1_000, 0.0);
        durations.push(began.elapsed().as_secs_f64() * 1_000.0);
    }
    durations.sort_by(f64::total_cmp);
    eprintln!(
        "mysql/mariadb monitor analysis: 1000 rows p95={:.3}ms (budget 10ms), retained={}bytes",
        durations[95],
        store.retained_bytes()
    );
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "aggregate server statement durations; no raw SQL"
)]
async fn server_durations(monitor: &mut MySqlSession) -> TestResult {
    let sample = monitor.read(Source::Statements, 1_000).await;
    let mut calls = 0_u64;
    let mut total = 0.0;
    let mut max: f64 = 0.0;
    for row in &sample.rows {
        let number = |index: usize| {
            row.get(index)
                .and_then(Option::as_deref)
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or_default()
        };
        calls += row
            .get(1)
            .and_then(Option::as_deref)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default();
        total += number(2);
        max = max.max(number(4));
    }
    eprintln!(
        "mysql/mariadb monitor server: covered fixture statements calls={calls}, total={total:.3}ms, max={max:.3}ms (includes fixture and guard commands; no baseline throughput claim)"
    );
    monitor.execute("SELECT 1", &mut Collect::default()).await?;
    Ok(())
}
