// SPDX-License-Identifier: Apache-2.0
//! A real streaming primary/replica pair: sender lag on the primary, receiver on the
//! standby. `just test-pg` starts the pair as one serial phase.
use super::monitor::sql;
use super::monitor_fixture::{require_phase, series};
use super::support::{self, TestResult};
use openrows_driver_postgres::{ConnectOptions, PgSession};
use openrows_monitor::{LiveStore, MonitorReader, Reading, SampleState, Source};
use std::time::Instant;

async fn session(port_variable: &str) -> TestResult<PgSession> {
    let mut config = support::config()?;
    config.port = std::env::var(port_variable)?.parse()?;
    Ok(PgSession::connect_with(
        &config,
        &ConnectOptions {
            application_name: "OpenRows monitor".into(),
            ..ConnectOptions::default()
        },
    )
    .await?)
}

/// Largest value of a lag column across sender rows; `None` while unknown.
fn lag(reading: &Reading, column: usize) -> Option<f64> {
    reading
        .rows
        .iter()
        .filter_map(|row| row.get(column)?.as_deref()?.parse::<f64>().ok())
        .max_by(f64::total_cmp)
}

#[tokio::test]
#[ignore = "needs the primary/replica PostgreSQL pair (just test-pg phase)"]
async fn streaming_replica_lag_on_primary_and_receiver_on_standby() -> TestResult {
    require_phase("replica")?;
    let mut writer = support::connect().await?;
    sql(
        &mut writer,
        "CREATE TABLE IF NOT EXISTS replica_fixture (id serial PRIMARY KEY, pad text)",
    )
    .await?;
    let mut primary = session("OPENROWS_TEST_PG_PORT").await?;
    let mut standby = session("OPENROWS_TEST_PG_REPLICA_PORT").await?;
    primary.discover().await;
    standby.discover().await;
    let sample = replay_lag(&mut writer, &mut primary, &mut standby).await?;
    assert!(sample.rows.iter().any(|row| {
        row.first()
            .and_then(Option::as_deref)
            .is_some_and(|id| id.starts_with("sender:"))
            && row.get(1).and_then(Option::as_deref) == Some("streaming")
    }));
    let mut store = LiveStore::default();
    store.ingest(
        Source::Replication,
        sample,
        0,
        super::monitor_capture::wall_ms(),
    );
    assert!(series(&store)?.iter().any(|(name, value, state)| {
        name == "Replication:replay_lag_ms"
            && state == "ok"
            && value.is_some_and(|ms| ms >= 1_000.0)
    }));
    receiver(&mut standby).await
}

/// The standby applies commits 2 s late (`recovery_min_apply_delay`), so while the primary
/// keeps writing it reports about 2 s of replay lag next to a small, known write lag.
async fn replay_lag(
    writer: &mut PgSession,
    primary: &mut PgSession,
    standby: &mut PgSession,
) -> TestResult<Reading> {
    let deadline = Instant::now();
    loop {
        sql(writer, "INSERT INTO replica_fixture (pad) SELECT repeat('x', 100) FROM generate_series(1, 100)").await?;
        assert_eq!(
            standby.read(Source::Replication, 1_000).await.state,
            SampleState::Ok
        );
        let sample = primary.read(Source::Replication, 1_000).await;
        assert_eq!(sample.state, SampleState::Ok);
        let delayed = lag(&sample, 5).is_some_and(|ms| ms >= 1_000.0);
        if delayed && lag(&sample, 3).is_some() {
            return Ok(sample);
        }
        if deadline.elapsed().as_secs() >= 30 {
            return Err(format!("no replay lag reported: {:?}", sample.rows).into());
        }
    }
}

/// The standby reports its WAL receiver; lag columns are unknown there, never zero.
async fn receiver(standby: &mut PgSession) -> TestResult {
    let sample = standby.read(Source::Replication, 1_000).await;
    assert_eq!(sample.state, SampleState::Ok);
    assert!(sample.rows.iter().any(|row| {
        row.first()
            .and_then(Option::as_deref)
            .is_some_and(|id| id.starts_with("receiver:"))
            && row.get(1).and_then(Option::as_deref) == Some("streaming")
    }));
    let mut store = LiveStore::default();
    store.ingest(
        Source::Replication,
        sample,
        0,
        super::monitor_capture::wall_ms(),
    );
    assert!(series(&store)?.iter().any(|(name, value, state)| {
        name == "Replication:write_lag_ms" && value.is_none() && state == "unknown"
    }));
    assert_eq!(
        standby.read(Source::Database, 1_000).await.state,
        SampleState::Ok
    );
    Ok(())
}
