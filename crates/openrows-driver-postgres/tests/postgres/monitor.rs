// SPDX-License-Identifier: Apache-2.0
//! One isolated fixture covers activity/blocking, rates, grants and optional statistics.
use super::support::{self, Collect, TestResult};
use arrow_array::{Array, Float64Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_driver_api::{Connection, Session};
use openrows_driver_postgres::{ConnectOptions, PgSession};
use openrows_monitor::{LiveStore, MonitorReader, Reading, SampleState, Source};
use std::{io::Cursor, time::Instant};

async fn sql(session: &mut PgSession, sql: &str) -> TestResult {
    for statement in openrows_sql::split(sql, openrows_sql::Dialect::Postgres) {
        session
            .execute(statement.text(sql), &mut Collect::default())
            .await?;
    }
    Ok(())
}
/// Connect only to the isolated monitoring fixture database.
pub(super) async fn connection(database: &str, role: Option<&str>) -> TestResult<PgSession> {
    let mut config = support::config()?;
    config.database = database.into();
    if let Some(role) = role {
        config.user = role.into();
        config.password = Some("pw".into());
    }
    Ok(PgSession::connect_with(
        &config,
        &ConnectOptions {
            application_name: "monitor-fixture".into(),
            ..ConnectOptions::default()
        },
    )
    .await?)
}

#[tokio::test]
#[ignore = "needs the disposable PostgreSQL container (just test-pg)"]
async fn monitoring_vertical_slice_activity_blockers_rates_extensions_and_permissions() -> TestResult
{
    let mut admin = support::connect().await?;
    let database = format!("openrows_monitor_test_{}", std::process::id());
    let role = format!("monitor_test_{}", std::process::id());
    sql(&mut admin, &format!("CREATE DATABASE {database}")).await?;
    sql(
        &mut admin,
        &format!("CREATE ROLE {role} LOGIN PASSWORD 'pw'"),
    )
    .await?;
    let result = exercise((&database, &role)).await;
    sql(
        &mut admin,
        &format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database}'"
        ),
    )
    .await?;
    sql(&mut admin, &format!("DROP DATABASE {database}")).await?;
    sql(&mut admin, &format!("DROP ROLE {role}")).await?;
    result
}

async fn exercise((database, role): (&str, &str)) -> TestResult {
    let mut writer = connection(database, None).await?;
    sql(&mut writer, "CREATE TABLE monitor_fixture (id int PRIMARY KEY, value text); INSERT INTO monitor_fixture VALUES (1, 'test-only')").await?;
    let mut monitor = connection(database, None).await?;
    sql(&mut monitor, "SET application_name = 'OpenRows monitor'").await?;
    extensions(&mut writer, &mut monitor).await?;
    privileges(&mut writer, database, role).await?;
    blocking(&mut writer, &mut monitor, database).await?;
    counters(&mut monitor, database).await?;
    super::monitor_reset::verify(&mut monitor, database).await?;
    resources(&mut monitor).await?;
    statements(&mut monitor, &mut writer).await?;
    if std::env::var_os("OPENROWS_BENCH").is_some() {
        super::monitor_bench::measure(&mut monitor).await?;
    }
    Ok(())
}

async fn extensions(writer: &mut PgSession, monitor: &mut PgSession) -> TestResult {
    let absent = monitor.discover().await;
    assert!(absent.iter().any(|d| d.source == Source::Statements
        && matches!(&d.state, SampleState::Unavailable(reason) if reason.contains("not created"))));
    sql(writer, "CREATE EXTENSION pg_stat_statements").await?;
    let present = monitor.discover().await;
    assert!(
        present
            .iter()
            .any(|d| d.source == Source::Statements && d.state == SampleState::Ok)
    );
    Ok(())
}

async fn privileges(writer: &mut PgSession, database: &str, role: &str) -> TestResult {
    let mut restricted = connection(database, Some(role)).await?;
    let permissions = restricted.discover().await;
    for source in [Source::Activity, Source::Statements, Source::Replication] {
        assert!(
            permissions
                .iter()
                .any(|d| d.source == source && matches!(d.state, SampleState::PermissionDenied(_)))
        );
        assert!(matches!(
            restricted.read(source, 1_000).await.state,
            SampleState::PermissionDenied(_)
        ));
    }
    sql(writer, &format!("GRANT pg_read_all_stats TO {role}")).await?;
    assert!(
        restricted
            .discover()
            .await
            .iter()
            .any(|d| d.source == Source::Activity && d.state == SampleState::Ok)
    );
    sql(writer, &format!("REVOKE pg_read_all_stats FROM {role}")).await?;
    assert!(matches!(
        restricted.read(Source::Activity, 1_000).await.state,
        SampleState::PermissionDenied(_)
    ));
    Ok(())
}

async fn blocking(writer: &mut PgSession, monitor: &mut PgSession, database: &str) -> TestResult {
    sql(
        writer,
        "BEGIN; UPDATE monitor_fixture SET value = 'test-only' WHERE id = 1",
    )
    .await?;
    let blocker = writer
        .admin()
        .ok_or("admin facet")?
        .own_process()
        .await?
        .to_string();
    let mut waiter = connection(database, None).await?;
    let waiter_pid = waiter
        .admin()
        .ok_or("admin facet")?
        .own_process()
        .await?
        .to_string();
    let pending = tokio::spawn(async move {
        sql(
            &mut waiter,
            "UPDATE monitor_fixture SET value = 'test-only' WHERE id = 1 /* test-only */",
        )
        .await
        .map_err(|_| "waiter failed")
    });
    let mut observed = None;
    let deadline = Instant::now();
    while deadline.elapsed().as_secs() < 5 {
        let sample = monitor.read(Source::Activity, 1_000).await;
        if sample.rows.iter().any(|row| {
            row.first().and_then(Option::as_deref) == Some(&waiter_pid)
                && row
                    .get(5)
                    .and_then(Option::as_deref)
                    .is_some_and(|ids| ids.contains(&blocker))
        }) {
            observed = Some(sample);
            break;
        }
    }
    sql(writer, "ROLLBACK").await?;
    pending.await??;
    let sample = observed.ok_or("blocking pair was not observed")?;
    assert!(
        sample
            .rows
            .iter()
            .any(|row| row.get(1).and_then(Option::as_deref) == Some("active"))
    );
    let mut store = LiveStore::default();
    store.ingest(Source::Activity, sample, 0, 0.0);
    let bytes = store.window(Some(Source::Activity), 1_000, false)?;
    assert!(!String::from_utf8_lossy(&bytes).contains("test-only"));
    let batch = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let chains = batch
        .column_by_name("blocking_chain")
        .ok_or("chain column")?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("chain type")?;
    assert!(
        chains
            .iter()
            .flatten()
            .any(|chain| chain.contains(&blocker) && chain.contains(&waiter_pid))
    );
    Ok(())
}

async fn counters(monitor: &mut PgSession, database: &str) -> TestResult {
    let first = monitor.read(Source::Database, 1_000).await;
    let before = number(&first, 2)?;
    let mut work = connection(database, None).await?;
    for _ in 0..20 {
        sql(&mut work, "SELECT count(*) FROM monitor_fixture").await?;
    }
    Box::new(work).close().await?;
    let deadline = Instant::now();
    let mut second = monitor.read(Source::Database, 1_000).await;
    while number(&second, 2)? <= before && deadline.elapsed().as_secs() < 5 {
        second = monitor.read(Source::Database, 1_000).await;
    }
    assert!(number(&second, 2)? > before);
    let mut store = LiveStore::default();
    store.ingest(Source::Database, first, 0, 0.0);
    store.ingest(Source::Database, second, 1_000, 1_000.0);
    let bytes = store.window(None, 1_000, false)?;
    let batch = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let names = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("metric names")?;
    let rates = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("rates")?;
    assert!(names.iter().enumerate().any(|(index, name)| {
        name.is_some_and(|name| name.ends_with(":xact_commit"))
            && !rates.is_null(index)
            && rates.value(index) > 0.0
    }));
    Ok(())
}
fn number(reading: &Reading, column: usize) -> TestResult<u64> {
    Ok(reading
        .rows
        .first()
        .and_then(|row| row.get(column))
        .and_then(Option::as_deref)
        .ok_or("missing counter")?
        .parse()?)
}

async fn resources(monitor: &mut PgSession) -> TestResult {
    for source in [Source::Tables, Source::Indexes] {
        let sample = monitor.read(source, 1_000).await;
        assert_eq!(sample.state, SampleState::Ok);
        assert!(!sample.rows.is_empty());
        assert!(!monitor.in_transaction());
    }
    let replication = monitor.read(Source::Replication, 1_000).await;
    assert!(matches!(replication.state, SampleState::Unavailable(_)));
    Ok(())
}

async fn statements(monitor: &mut PgSession, writer: &mut PgSession) -> TestResult {
    sql(writer, "SELECT count(*) FROM monitor_fixture").await?;
    let sample = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(sample.state, SampleState::Ok);
    assert!(!sample.rows.is_empty());
    assert!(sample.columns.iter().all(|c| c.name != "query"));
    sql(writer, "SET pg_stat_statements.track = 'none'").await?;
    // Instrumentation settings are per session: configure the collector fixture itself.
    sql(monitor, "SET pg_stat_statements.track = 'none'").await?;
    assert!(monitor.discover().await.iter().any(|d| d.source == Source::Statements && matches!(&d.state, SampleState::Unavailable(reason) if reason.contains("disabled"))));
    sql(monitor, "SET pg_stat_statements.track = 'top'").await?;
    monitor.discover().await;
    Ok(())
}
