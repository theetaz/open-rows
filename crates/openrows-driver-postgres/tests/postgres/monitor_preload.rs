// SPDX-License-Identifier: Apache-2.0
//! `pg_stat_statements` created in a database on a server that does not preload it.
//! `just test-pg` runs this in its own container without `shared_preload_libraries`.
use super::monitor::{connection, sql};
use super::monitor_fixture::require_phase;
use super::support::{self, TestResult};
use openrows_driver_postgres::PgSession;
use openrows_monitor::{MonitorReader, SampleState, Source};

const NOT_PRELOADED: &str = "installed but not preloaded";

#[tokio::test]
#[ignore = "needs the preload-free PostgreSQL container (just test-pg phase)"]
async fn preload_absent_statement_statistics_are_typed_unavailable() -> TestResult {
    require_phase("preload-absent")?;
    let mut admin = support::connect().await?;
    let database = format!("openrows_monitor_preload_{}", std::process::id());
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
    sql(&mut writer, "CREATE EXTENSION pg_stat_statements").await?;
    let mut monitor = connection(database, None).await?;
    sql(&mut monitor, "SET application_name = 'OpenRows monitor'").await?;
    expect_not_preloaded(&mut monitor).await?;
    // Loading the module into the collector's own session must not hide the state.
    sql(&mut monitor, "LOAD 'pg_stat_statements'").await?;
    expect_not_preloaded(&mut monitor).await?;
    let database = monitor.read(Source::Database, 1_000).await;
    assert_eq!(database.state, SampleState::Ok, "other sources still work");
    Ok(())
}

async fn expect_not_preloaded(monitor: &mut PgSession) -> TestResult {
    let descriptors = monitor.discover().await;
    let statements = descriptors
        .iter()
        .find(|d| d.source == Source::Statements)
        .ok_or("statement descriptor")?;
    assert!(
        matches!(&statements.state, SampleState::Unavailable(reason) if reason.contains(NOT_PRELOADED)),
        "discovery: {:?}",
        statements.state
    );
    let reading = monitor.read(Source::Statements, 1_000).await;
    assert!(
        matches!(&reading.state, SampleState::Unavailable(reason) if reason.contains(NOT_PRELOADED)),
        "read: {:?}",
        reading.state
    );
    Ok(())
}
