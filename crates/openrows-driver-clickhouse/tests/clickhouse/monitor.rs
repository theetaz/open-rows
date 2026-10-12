// SPDX-License-Identifier: Apache-2.0
//! Live collector evidence; fixture setup (DDL, users, log flush) is administrative only.
use super::support::{TestResult, config};
use openrows_driver_api::{ConnectTrace, DiscardSink, Session};
use openrows_driver_clickhouse::{ClickHouseSession, connect};
use openrows_monitor::{Descriptor, MonitorReader, Reading, SampleState, Source};

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn counters_queries_storage_and_scope() -> TestResult {
    let mut admin = connect(&config()?, &mut ConnectTrace::default()).await?;
    run(
        &mut admin,
        &[
            "DROP TABLE IF EXISTS monitor_fixture",
            "CREATE TABLE monitor_fixture(id UInt64, value String) ENGINE = MergeTree ORDER BY id",
            "INSERT INTO monitor_fixture VALUES (1, 'test-only')",
            "INSERT INTO monitor_fixture VALUES (2, 'test-only')",
            "SELECT count() FROM monitor_fixture WHERE value = 'test-only'",
        ],
    )
    .await?;
    let mut monitor = connect(&config()?, &mut ConnectTrace::default()).await?;
    let descriptors = monitor.discover().await;
    let log = std::env::var("OPENROWS_TEST_CLICKHOUSE_QUERY_LOG").as_deref() != Ok("OFF");
    for source in [
        Source::Activity,
        Source::Database,
        Source::Tables,
        Source::Io,
    ] {
        assert_eq!(state(&descriptors, source), SampleState::Ok, "{source:?}");
    }
    assert!(coverage(&descriptors, Source::Activity).starts_with("Node scope"));
    assert!(matches!(
        state(&descriptors, Source::Indexes),
        SampleState::Unavailable(_)
    ));
    counters(&mut monitor).await?;
    activity(&mut monitor).await?;
    storage(&mut monitor).await?;
    if log {
        run(&mut admin, &["SYSTEM FLUSH LOGS"]).await?;
        statements(&mut monitor).await?;
    } else {
        assert!(matches!(
            state(&descriptors, Source::Statements),
            SampleState::Unavailable(reason) if reason.contains("query_log is not configured")
        ));
        assert!(
            monitor
                .read(Source::Statements, 1_000)
                .await
                .rows
                .is_empty()
        );
    }
    assert!(matches!(
        monitor.read(Source::Replication, 1_000).await.state,
        SampleState::Unavailable(reason) if reason.contains("Not applicable")
    ));
    cluster().await?;
    run(&mut admin, &["DROP TABLE monitor_fixture"]).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs ClickHouse; run just test-clickhouse"]
async fn restricted_grants_stay_explicit() -> TestResult {
    let mut admin = connect(&config()?, &mut ConnectTrace::default()).await?;
    run(
        &mut admin,
        &[
            "DROP USER IF EXISTS monitor_low",
            "CREATE USER monitor_low IDENTIFIED BY 'test-only-low'",
            "GRANT SELECT ON system.processes TO monitor_low",
            "GRANT SELECT ON system.events TO monitor_low",
            "GRANT SELECT ON system.one TO monitor_low",
        ],
    )
    .await?;
    let mut limited = config()?;
    limited.user = "monitor_low".into();
    limited.password = Some("test-only-low".into());
    limited
        .driver_options
        .insert("monitor_cluster".into(), "default".into());
    let mut monitor = connect(&limited, &mut ConnectTrace::default()).await?;
    let descriptors = monitor.discover().await;
    assert_eq!(state(&descriptors, Source::Activity), SampleState::Ok);
    assert!(coverage(&descriptors, Source::Activity).contains("REMOTE is not granted"));
    // A missing query_log is reported before grants are checked.
    let log = std::env::var("OPENROWS_TEST_CLICKHOUSE_QUERY_LOG").as_deref() != Ok("OFF");
    assert!(matches!(
        (log, state(&descriptors, Source::Statements)),
        (true, SampleState::PermissionDenied(_)) | (false, SampleState::Unavailable(_))
    ));
    for source in [Source::Tables, Source::Io, Source::Replication] {
        assert!(
            matches!(
                state(&descriptors, source),
                SampleState::PermissionDenied(_)
            ),
            "{source:?}: {descriptors:?}"
        );
    }
    assert_eq!(state(&descriptors, Source::Database), SampleState::Ok);
    assert!(coverage(&descriptors, Source::Database).contains("system.metrics is not readable"));
    let reading = monitor.read(Source::Database, 1_000).await;
    assert_eq!(cell(&reading, "running_queries"), None, "unknown, not zero");
    assert!(cell(&reading, "queries").is_some());
    assert!(monitor.read(Source::Tables, 1_000).await.rows.is_empty());
    run(&mut admin, &["DROP USER monitor_low"]).await?;
    Ok(())
}

async fn counters(monitor: &mut ClickHouseSession) -> TestResult {
    let first = monitor.read(Source::Database, 1_000).await;
    let second = monitor.read(Source::Database, 1_000).await;
    assert_eq!(second.state, SampleState::Ok);
    assert_eq!(first.epoch, second.epoch, "no reset between reads");
    let queries = |r: &Reading| cell(r, "queries").and_then(|v| v.parse::<u64>().ok());
    assert!(
        queries(&second) > queries(&first),
        "events counter advances"
    );
    assert!(cell(&second, "running_queries").is_some());
    assert!(cell(&second, "max_parts_per_partition").is_some());
    assert!(
        cell(&second, "insert_queries").is_some(),
        "zero events are listed"
    );
    Ok(())
}

async fn activity(monitor: &mut ClickHouseSession) -> TestResult {
    let mut sleeper = connect(&config()?, &mut ConnectTrace::default()).await?;
    let task = tokio::spawn(async move {
        let sql = "SELECT sleepEachRow(1) FROM numbers(3) SETTINGS max_block_size = 1";
        sleeper.execute(sql, &mut DiscardSink).await
    });
    let mut seen = false;
    for _ in 0..500 {
        let reading = monitor.read(Source::Activity, 1_000).await;
        assert_eq!(reading.state, SampleState::Ok);
        seen = reading.rows.iter().any(|row| {
            row.get(4)
                .and_then(Option::as_deref)
                .is_some_and(|q| q.contains("sleepEachRow"))
        });
        assert!(
            reading
                .rows
                .iter()
                .all(|row| row.get(8).is_some_and(Option::is_some))
        );
        if seen {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(seen, "running query visible");
    task.await??;
    Ok(())
}

async fn storage(monitor: &mut ClickHouseSession) -> TestResult {
    let tables = monitor.read(Source::Tables, 1_000).await;
    assert_eq!(tables.state, SampleState::Ok);
    let row = tables
        .rows
        .iter()
        .find(|r| r.first().cloned().flatten().as_deref() == Some("openrows_test.monitor_fixture"))
        .ok_or("parts row missing")?;
    let parts: u64 = row.get(3).cloned().flatten().ok_or("parts")?.parse()?;
    assert!(parts >= 1);
    assert_eq!(monitor.read(Source::Io, 1_000).await.state, SampleState::Ok);
    Ok(())
}

async fn statements(monitor: &mut ClickHouseSession) -> TestResult {
    let reading = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(reading.state, SampleState::Ok, "{reading:?}");
    let row = reading
        .rows
        .iter()
        .find(|r| {
            r.last()
                .and_then(Option::as_deref)
                .is_some_and(|q| q.contains("monitor_fixture") && q.starts_with("SELECT"))
        })
        .ok_or("query log fixture missing")?;
    assert!(
        !format!("{row:?}").contains("test-only"),
        "literals normalised"
    );
    assert!(
        reading
            .rows
            .iter()
            .all(|r| r.len() == reading.columns.len())
    );
    assert!(!reading.epoch.is_empty());
    Ok(())
}

async fn cluster() -> TestResult {
    let mut request = config()?;
    request
        .driver_options
        .insert("monitor_cluster".into(), "default".into());
    let mut monitor = connect(&request, &mut ConnectTrace::default()).await?;
    let descriptors = monitor.discover().await;
    assert!(coverage(&descriptors, Source::Activity).starts_with("Cluster scope"));
    assert_eq!(
        monitor.read(Source::Activity, 1_000).await.state,
        SampleState::Ok
    );
    Ok(())
}

async fn run(session: &mut ClickHouseSession, statements: &[&str]) -> TestResult {
    for sql in statements {
        session.execute(sql, &mut DiscardSink).await?;
    }
    Ok(())
}
fn state(descriptors: &[Descriptor], source: Source) -> SampleState {
    descriptors
        .iter()
        .find(|d| d.source == source)
        .map_or(SampleState::Unknown, |d| d.state.clone())
}
fn coverage(descriptors: &[Descriptor], source: Source) -> String {
    descriptors
        .iter()
        .find(|d| d.source == source)
        .map(|d| d.coverage.clone())
        .unwrap_or_default()
}
fn cell(reading: &Reading, name: &str) -> Option<String> {
    let index = reading.columns.iter().position(|c| c.name == name)?;
    reading.rows.first()?.get(index).cloned().flatten()
}
