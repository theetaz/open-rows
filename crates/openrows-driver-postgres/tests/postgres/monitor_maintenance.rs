// SPDX-License-Identifier: Apache-2.0
//! Real temp-file spill and vacuum/analyze work, observed by the collector in an
//! isolated database.
use super::monitor::{connection, sql};
use super::monitor_fixture::{cell, count, number, series, server_version};
use super::support::{self, TestResult};
use arrow_array::Array;
use openrows_driver_postgres::PgSession;
use openrows_monitor::{LiveStore, MonitorReader, Reading, SampleState, Source};
use std::time::Instant;

const SPILL: &str = "SELECT count(DISTINCT g) FROM generate_series(1, 300000) g";

#[tokio::test]
#[ignore = "needs the disposable PostgreSQL container (just test-pg)"]
async fn monitoring_observes_temp_spill_and_vacuum_counters() -> TestResult {
    let mut admin = support::connect().await?;
    let database = format!("openrows_monitor_maintenance_{}", std::process::id());
    sql(&mut admin, &format!("CREATE DATABASE {database}")).await?;
    let result = exercise(&database).await;
    sql(
        &mut admin,
        &format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database}'"
        ),
    )
    .await?;
    sql(&mut admin, &format!("DROP DATABASE {database}")).await?;
    result
}

async fn exercise(database: &str) -> TestResult {
    let mut writer = connection(database, None).await?;
    let flush = if server_version(&mut writer).await? >= 150_000 {
        "SELECT pg_stat_force_next_flush()"
    } else {
        "SELECT 1"
    };
    sql(&mut writer, "CREATE EXTENSION pg_stat_statements").await?;
    settle(&mut writer, flush).await?;
    let mut monitor = connection(database, None).await?;
    sql(&mut monitor, "SET application_name = 'OpenRows monitor'").await?;
    monitor.discover().await;
    spill(&mut writer, &mut monitor, flush).await?;
    vacuum(&mut writer, &mut monitor, flush).await
}

/// Give the fresh database a statistics entry with a reset time before the baseline. On
/// 13/14 the collector creates the entry asynchronously and sets `stats_reset`, which the
/// collector correctly reports as one counter reset; the fixture waits it out instead.
async fn settle(writer: &mut PgSession, flush: &str) -> TestResult {
    sql(writer, "SELECT pg_stat_reset()").await?;
    let deadline = Instant::now();
    loop {
        sql(writer, flush).await?;
        let batch = support::single_batch(
            writer,
            "SELECT (stats_reset IS NOT NULL)::text AS ready FROM pg_stat_database WHERE datname = current_database()",
        )
        .await?;
        let ready = support::column(&batch, "ready")?;
        let ready = ready
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .ok_or("ready column")?;
        if ready.value(0) == "true" {
            return Ok(());
        }
        if deadline.elapsed().as_secs() >= 10 {
            return Err("database statistics entry never appeared".into());
        }
    }
}

fn first(reading: &Reading, column: usize) -> f64 {
    reading
        .rows
        .first()
        .and_then(|row| row.get(column))
        .and_then(Option::as_deref)
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1.0)
}

/// A sort beyond `work_mem` writes temp files: database counters, their rates and the
/// statement's temp blocks all show it.
async fn spill(writer: &mut PgSession, monitor: &mut PgSession, flush: &str) -> TestResult {
    let began = Instant::now();
    let before = monitor.read(Source::Database, 1_000).await;
    assert_eq!(before.state, SampleState::Ok);
    sql(writer, &format!("SET work_mem = '64kB'; {SPILL}")).await?;
    let deadline = Instant::now();
    let mut after = monitor.read(Source::Database, 1_000).await;
    while first(&after, 6) <= first(&before, 6) && deadline.elapsed().as_secs() < 20 {
        sql(writer, flush).await?;
        after = monitor.read(Source::Database, 1_000).await;
    }
    assert!(first(&after, 6) > first(&before, 6), "temp_files grew");
    assert!(first(&after, 7) > first(&before, 7), "temp_bytes grew");
    let mut store = LiveStore::default();
    store.ingest(
        Source::Database,
        before,
        0,
        super::monitor_capture::wall_ms(),
    );
    let elapsed = u64::try_from(began.elapsed().as_millis())?.max(1);
    store.ingest(
        Source::Database,
        after,
        elapsed,
        super::monitor_capture::wall_ms(),
    );
    let rates = series(&store)?;
    assert!(
        rates.iter().any(|(name, value, state)| {
            name.ends_with(":temp_bytes") && state == "ok" && value.is_some_and(|rate| rate > 0.0)
        }),
        "{rates:?}"
    );
    let statements = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(statements.state, SampleState::Ok);
    assert!(statements.rows.iter().any(|row| {
        row.get(12)
            .and_then(Option::as_deref)
            .is_some_and(|query| query.contains("count(DISTINCT g)"))
            && row
                .get(11)
                .and_then(Option::as_deref)
                .and_then(|blocks| blocks.parse::<u64>().ok())
                .is_some_and(|blocks| blocks > 0)
    }));
    Ok(())
}

/// Dead tuples appear, then a manual VACUUM (ANALYZE) clears them and advances the
/// vacuum/analyze counters and ages; autovacuum is off for the fixture table. Backends
/// publish statistics when idle (up to 10 s on 15+), so the writer forces a flush.
async fn vacuum(writer: &mut PgSession, monitor: &mut PgSession, flush: &str) -> TestResult {
    sql(writer, "CREATE TABLE maintenance_fixture (id int PRIMARY KEY, pad text) WITH (autovacuum_enabled = false); INSERT INTO maintenance_fixture SELECT g, repeat('x', 100) FROM generate_series(1, 2000) g; DELETE FROM maintenance_fixture WHERE id % 2 = 0").await?;
    let table = (2, "maintenance_fixture");
    let deadline = Instant::now();
    let mut dirty = monitor.read(Source::Tables, 1_000).await;
    while count(&dirty, table, 6) < 1_000 && deadline.elapsed().as_secs() < 20 {
        sql(writer, flush).await?;
        dirty = monitor.read(Source::Tables, 1_000).await;
    }
    assert_eq!(count(&dirty, table, 6), 1_000, "dead tuples observed");
    assert_eq!(count(&dirty, table, 11), 0, "not vacuumed yet");
    assert_eq!(cell(&dirty, table, 9), None, "no manual vacuum yet");
    sql(writer, "VACUUM (ANALYZE) maintenance_fixture").await?;
    let deadline = Instant::now();
    let mut clean = monitor.read(Source::Tables, 1_000).await;
    while count(&clean, table, 11) < 1 && deadline.elapsed().as_secs() < 20 {
        sql(writer, flush).await?;
        clean = monitor.read(Source::Tables, 1_000).await;
    }
    assert_eq!(count(&clean, table, 6), 0, "dead tuples cleared");
    assert_eq!(count(&clean, table, 11), 1, "vacuum_count");
    assert_eq!(count(&clean, table, 13), 1, "analyze_count");
    assert!(cell(&clean, table, 9).is_some() && cell(&clean, table, 10).is_some());
    assert!(number(&clean, table, 19) >= 0.0, "vacuum age is known");
    assert!(number(&clean, table, 18) >= 0.0, "analyze age is known");
    Ok(())
}
