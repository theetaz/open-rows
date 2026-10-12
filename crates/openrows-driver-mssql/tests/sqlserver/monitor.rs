// SPDX-License-Identifier: Apache-2.0
//! DMV collectors against a real server: requests/blocking, waits with a
//! cleared baseline, counters, top queries, usage and a restricted login.
use openrows_driver_api::{Admin, ConnectTrace, Connection, Session, SslMode};
use openrows_driver_mssql::{SqlServerSession, connect};
use openrows_monitor::{Counter, MonitorReader, Reading, SampleState, Source, counter_delta};

use crate::support::{self, Collect, TestResult, ensure_database, fixture, run};

fn column(reading: &Reading, name: &str) -> Result<usize, String> {
    reading
        .columns
        .iter()
        .position(|c| c.name == name)
        .ok_or_else(|| format!("column {name}"))
}

fn cell<'a>(reading: &'a Reading, row: &'a [Option<String>], name: &str) -> Option<&'a str> {
    row.get(column(reading, name).ok()?)?.as_deref()
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn requests_waits_counters_queries_and_restricted_state() -> TestResult {
    let table = fixture("monitor");
    ensure_database().await?;
    let mut admin = support::session().await?;
    run(
        &mut admin,
        &[
            &format!("CREATE TABLE {table} (id int PRIMARY KEY, v int NOT NULL)"),
            &format!("INSERT INTO {table} VALUES (1, 0), (2, 0)"),
        ],
    )
    .await?;
    let mut monitor = support::session().await?;
    assert!(monitor.monitor().is_some());
    let descriptors = monitor.discover().await;
    for descriptor in &descriptors {
        let expected = if descriptor.source == Source::Replication {
            matches!(descriptor.state, SampleState::Unavailable(_))
        } else {
            descriptor.state == SampleState::Ok
        };
        assert!(expected, "{:?}: {:?}", descriptor.source, descriptor.state);
    }
    blocking(&mut admin, &mut monitor, &table).await?;
    waits(&mut admin, &mut monitor).await?;
    counters_and_usage(&mut admin, &mut monitor, &table).await?;
    restricted(&mut admin).await?;
    run(&mut admin, &[&format!("DROP TABLE {table}")]).await?;
    Ok(())
}

async fn blocking(
    admin: &mut SqlServerSession,
    monitor: &mut SqlServerSession,
    table: &str,
) -> TestResult {
    let mut holder = support::session().await?;
    let holder_id = holder.own_process().await?.to_string();
    run(
        &mut holder,
        &[&format!(
            "BEGIN TRANSACTION; UPDATE {table} SET v = v + 1 WHERE id = 1"
        )],
    )
    .await?;
    let mut waiter = support::session().await?;
    let waiter_id = waiter.own_process().await?.to_string();
    let sql = format!("UPDATE {table} SET v = v + 1 WHERE id = 1");
    let blocked = tokio::spawn(async move {
        let result = waiter.execute(&sql, &mut Collect::default()).await;
        (waiter, result.map(drop))
    });
    let mut found = None;
    for _ in 0..200 {
        let reading = monitor.read(Source::Activity, 1_000).await;
        assert_eq!(reading.state, SampleState::Ok);
        let row = reading
            .rows
            .iter()
            .find(|row| row.first().and_then(Option::as_deref) == Some(&waiter_id))
            .filter(|row| cell(&reading, row, "blockers").is_some())
            .cloned();
        if let Some(row) = row {
            found = Some((reading, row));
            break;
        }
    }
    let (reading, row) = found.ok_or("blocked request not observed")?;
    assert_eq!(cell(&reading, &row, "blockers"), Some(holder_id.as_str()));
    assert_eq!(cell(&reading, &row, "state"), Some("active"));
    assert!(cell(&reading, &row, "wait").is_some_and(|w| w.starts_with("LCK_M_")));
    let holder_row = reading
        .rows
        .iter()
        .find(|row| row.first().and_then(Option::as_deref) == Some(&holder_id))
        .ok_or("holder row")?;
    assert_eq!(
        cell(&reading, holder_row, "state"),
        Some("idle in transaction")
    );
    assert!(cell(&reading, holder_row, "transaction_ms").is_some());
    run(&mut holder, &["ROLLBACK"]).await?;
    let (waiter, result) = blocked.await?;
    result?;
    drop((waiter, holder, admin));
    Ok(())
}

fn wait_ms(reading: &Reading, wait: &str) -> Option<u64> {
    let row = reading
        .rows
        .iter()
        .find(|row| row.first().and_then(Option::as_deref) == Some(wait))?;
    cell(reading, row, "wait_ms")?.parse().ok()
}

/// Clearing the wait statistics keeps the epoch; the decrease is a reset.
async fn waits(admin: &mut SqlServerSession, monitor: &mut SqlServerSession) -> TestResult {
    let before = monitor.read(Source::Io, 1_000).await;
    assert_eq!(before.state, SampleState::Ok);
    assert!(!before.rows.is_empty());
    assert!(
        before.epoch.len() > 10,
        "start-time epoch: {}",
        before.epoch
    );
    let (top, previous) = before
        .rows
        .iter()
        .find_map(|row| {
            let name = row.first()?.as_deref()?;
            Some((name.to_owned(), wait_ms(&before, name)?))
        })
        .ok_or("top wait")?;
    assert!(previous > 0);
    run(admin, &["DBCC SQLPERF('sys.dm_os_wait_stats', CLEAR)"]).await?;
    let after = monitor.read(Source::Io, 1_000).await;
    assert_eq!(after.epoch, before.epoch);
    let current = wait_ms(&after, &top).unwrap_or(0);
    let counter = |value, elapsed_ms| Counter {
        value,
        epoch: before.epoch.clone(),
        elapsed_ms,
    };
    let (rate, state) = counter_delta(Some(&counter(previous, 0)), &counter(current, 1_000));
    assert!(current < previous);
    assert_eq!((rate, state), (None, SampleState::CounterReset));
    Ok(())
}

async fn counters_and_usage(
    admin: &mut SqlServerSession,
    monitor: &mut SqlServerSession,
    table: &str,
) -> TestResult {
    let database = monitor.read(Source::Database, 1_000).await;
    assert_eq!(database.state, SampleState::Ok);
    let row = database.rows.first().ok_or("counter row")?;
    for name in [
        "batch_requests",
        "user_connections",
        "total_server_memory_kb",
    ] {
        let value = cell(&database, row, name).ok_or(name)?;
        assert!(value.parse::<f64>().is_ok(), "{name}: {value}");
    }
    let query = format!("SELECT COUNT(*) FROM {table} WHERE v >= 0");
    for _ in 0..3 {
        admin.execute(&query, &mut Collect::default()).await?;
    }
    let statements = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(statements.state, SampleState::Ok);
    let row = statements
        .rows
        .iter()
        .find(|row| {
            cell(&statements, row, "query")
                .is_some_and(|q| q.contains(table) && q.contains("COUNT"))
        })
        .ok_or("statement row")?;
    let calls: u64 = cell(&statements, row, "calls").ok_or("calls")?.parse()?;
    assert!(calls >= 3);
    assert!(cell(&statements, row, "statement_id").is_some_and(|id| id.starts_with("0x")));
    let tables = monitor.read(Source::Tables, 1_000).await;
    assert!(
        tables
            .rows
            .iter()
            .any(|row| cell(&tables, row, "table") == Some(table))
    );
    let indexes = monitor.read(Source::Indexes, 1_000).await;
    assert_eq!(indexes.state, SampleState::Ok);
    assert_eq!(monitor.read(Source::Replication, 1_000).await.rows.len(), 0);
    Ok(())
}

/// A login without the state permission sees explicit denials, never zeros.
async fn restricted(admin: &mut SqlServerSession) -> TestResult {
    let login = fixture("monitor_low");
    let mut request = support::request(SslMode::Require)?;
    let password = format!(
        "{}-low",
        request.password.as_ref().ok_or("password")?.expose()
    );
    run(
        admin,
        &[
            &format!("CREATE LOGIN {login} WITH PASSWORD = '{password}', CHECK_POLICY = OFF"),
            &format!("CREATE USER {login} FOR LOGIN {login}"),
        ],
    )
    .await?;
    request.user.clone_from(&login);
    request.password = Some(password.as_str().into());
    let mut low = connect(&request, &mut ConnectTrace::default()).await?;
    for descriptor in low.discover().await {
        if descriptor.source == Source::Replication {
            continue;
        }
        assert!(
            matches!(&descriptor.state, SampleState::PermissionDenied(reason) if reason.contains("VIEW SERVER STATE")),
            "{:?}: {:?}",
            descriptor.source,
            descriptor.state
        );
        let reading = low.read(descriptor.source, 1_000).await;
        assert!(matches!(reading.state, SampleState::PermissionDenied(_)));
        assert!(reading.rows.is_empty());
    }
    Box::new(low).close().await?;
    run(
        admin,
        &[
            &format!("DROP USER {login}"),
            &format!("DROP LOGIN {login}"),
        ],
    )
    .await?;
    Ok(())
}
