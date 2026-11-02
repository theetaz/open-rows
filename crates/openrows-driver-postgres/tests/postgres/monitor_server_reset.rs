// SPDX-License-Identifier: Apache-2.0
//! Server-wide statement and I/O statistics resets by a fixture administrator.
//! `just test-pg` runs this test alone after the main suite: a full reset would race
//! other tests' statement reads.
use super::monitor::{connection, sql};
use super::monitor_fixture::server_version;
use super::support::{self, TestResult};
use openrows_driver_postgres::PgSession;
use openrows_monitor::{MonitorReader, SampleState, Source};
use std::time::Instant;

#[tokio::test]
#[ignore = "needs the disposable PostgreSQL container (just test-pg, serial phase)"]
async fn server_statistics_reset_changes_statement_and_io_epochs() -> TestResult {
    let mut admin = support::connect().await?;
    let database = format!("openrows_monitor_server_reset_{}", std::process::id());
    sql(&mut admin, &format!("CREATE DATABASE {database}")).await?;
    let result = exercise(&mut admin, &database).await;
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

async fn exercise(admin: &mut PgSession, database: &str) -> TestResult {
    let version = server_version(admin).await?;
    let mut writer = connection(database, None).await?;
    sql(&mut writer, "CREATE EXTENSION pg_stat_statements").await?;
    let mut monitor = connection(database, None).await?;
    sql(&mut monitor, "SET application_name = 'OpenRows monitor'").await?;
    let descriptors = monitor.discover().await;
    statements(&mut writer, &mut monitor, version).await?;
    let io = descriptors
        .iter()
        .find(|d| d.source == Source::Io)
        .ok_or("io descriptor")?;
    if version < 160_000 {
        assert!(
            matches!(&io.state, SampleState::Unavailable(reason) if reason.contains("16+")),
            "pg_stat_io is explicitly unavailable before 16"
        );
        return Ok(());
    }
    assert_eq!(io.state, SampleState::Ok);
    reset(
        (&mut writer, &mut monitor),
        Source::Io,
        "SELECT pg_stat_reset_shared('io')",
    )
    .await
}

/// `PostgreSQL` 14+ exposes `pg_stat_statements_info.stats_reset`, so a full reset changes
/// the statement epoch and rates restart; on 13 continuity stays best effort.
async fn statements(writer: &mut PgSession, monitor: &mut PgSession, version: i32) -> TestResult {
    sql(writer, "SELECT count(*) FROM pg_class").await?;
    let before = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(before.state, SampleState::Ok);
    assert!(!before.rows.is_empty());
    if version >= 140_000 {
        return reset(
            (writer, monitor),
            Source::Statements,
            "SELECT pg_stat_statements_reset()",
        )
        .await;
    }
    sql(writer, "SELECT pg_stat_statements_reset()").await?;
    let after = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(after.state, SampleState::Ok);
    assert_eq!(after.epoch, before.epoch, "13 has no statement reset epoch");
    Ok(())
}

/// Run `statement` from the fixture's administrator session (never the collector) and
/// wait for the source epoch to change.
async fn reset(
    (administrator, monitor): (&mut PgSession, &mut PgSession),
    source: Source,
    statement: &str,
) -> TestResult {
    let before = monitor.read(source, 1_000).await;
    assert_eq!(before.state, SampleState::Ok);
    sql(administrator, statement).await?;
    let deadline = Instant::now();
    let mut after = monitor.read(source, 1_000).await;
    while after.epoch == before.epoch && deadline.elapsed().as_secs() < 5 {
        after = monitor.read(source, 1_000).await;
    }
    assert_eq!(after.state, SampleState::Ok);
    assert_ne!(
        after.epoch, before.epoch,
        "{source:?} epoch changed on reset"
    );
    Ok(())
}
