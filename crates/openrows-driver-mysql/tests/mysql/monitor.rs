// SPDX-License-Identifier: Apache-2.0
//! Native `MySQL`/`MariaDB` source evidence; fixture-only setup is explicitly administrative.
use super::support::{self, Collect, run};
use arrow_array::{Array, Float64Array, RecordBatch, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_driver_api::{ConnectTrace, Connection, Session, SslMode};
use openrows_driver_mysql::{MySqlSession, connect};
use openrows_monitor::{Descriptor, LiveStore, MonitorReader, Reading, SampleState, Source};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "needs sequential MySQL/MariaDB test containers"]
async fn activity_blockers_rates_digest_and_restricted_grants() -> TestResult {
    let mut admin = support::session().await?;
    let instrumented = performance_schema(&mut admin).await?;
    run(
        &mut admin,
        &[
            "DROP TABLE IF EXISTS monitor_fixture",
            "CREATE TABLE monitor_fixture(id INT PRIMARY KEY, value VARCHAR(32)) ENGINE=InnoDB",
            "INSERT INTO monitor_fixture VALUES (1, 'test-only')",
        ],
    )
    .await?;
    if instrumented {
        run(&mut admin, &[
            "UPDATE performance_schema.setup_instruments SET ENABLED='YES', TIMED='YES' WHERE NAME LIKE 'statement/%' OR NAME LIKE 'wait/io/table/%' OR NAME LIKE 'wait/io/file/%'",
            "UPDATE performance_schema.setup_consumers SET ENABLED='YES' WHERE NAME IN ('global_instrumentation', 'thread_instrumentation', 'statements_digest')",
        ]).await?;
    }
    let mut monitor = support::session().await?;
    assert!(monitor.monitor().is_some());
    capabilities(&monitor.discover().await, instrumented);
    // MySQL InnoDB lock waits live only in performance_schema.data_lock_waits.
    if instrumented || support::mariadb() {
        blocking(&mut admin, &mut monitor).await?;
    }
    counters(&mut admin, &mut monitor, instrumented).await?;
    if instrumented {
        digests(&mut monitor).await?;
    }
    assert_eq!(
        monitor.read(Source::Replication, 1_000).await.state,
        SampleState::Unavailable("Not applicable: no replica channels".into())
    );
    restricted(&mut admin, instrumented).await?;
    if instrumented && std::env::var("OPENROWS_BENCH").as_deref() == Ok("1") {
        super::monitor_bench::measure(&mut monitor).await?;
    }
    run(
        &mut admin,
        &[
            "DROP USER IF EXISTS 'monitor_low'@'%'",
            "DROP TABLE monitor_fixture",
        ],
    )
    .await?;
    Ok(())
}
/// Whether the server started with `performance_schema=ON` (`MariaDB` defaults to OFF).
async fn performance_schema(admin: &mut MySqlSession) -> TestResult<bool> {
    let mut rows = Collect::default();
    admin
        .execute("SELECT 1 FROM DUAL WHERE @@performance_schema", &mut rows)
        .await?;
    Ok(rows.rows() == 1)
}
fn capabilities(descriptors: &[Descriptor], instrumented: bool) {
    let state = |source| state_of(descriptors, source);
    for source in [Source::Activity, Source::Database] {
        assert_eq!(
            state(source),
            Some(&SampleState::Ok),
            "{source:?}: {descriptors:?}"
        );
    }
    for source in [
        Source::Statements,
        Source::Tables,
        Source::Indexes,
        Source::Io,
    ] {
        if instrumented {
            assert_eq!(
                state(source),
                Some(&SampleState::Ok),
                "{source:?}: {descriptors:?}"
            );
        } else {
            assert!(
                matches!(state(source), Some(SampleState::Unavailable(reason)) if reason.contains("performance_schema disabled") && reason.contains("restart")),
                "{source:?}: {descriptors:?}"
            );
        }
    }
}
async fn digests(monitor: &mut MySqlSession) -> TestResult {
    let sample = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(sample.state, SampleState::Ok);
    assert!(
        sample.rows.iter().any(|r| r
            .get(9)
            .and_then(Option::as_deref)
            .is_some_and(|q| q.to_lowercase().contains("monitor_fixture"))),
        "digest fixture missing"
    );
    let mut store = LiveStore::default();
    store.ingest(Source::Statements, sample, 0, 0.0);
    let second = monitor.read(Source::Statements, 1_000).await;
    store.ingest(Source::Statements, second, 1000, 1000.0);
    let bytes = store.window(Some(Source::Statements), 20, false)?;
    assert!(!String::from_utf8_lossy(&bytes).contains("test-only"));
    assert!(batch(bytes)?.num_rows() > 0, "top-N window empty");
    Ok(())
}
async fn blocking(admin: &mut MySqlSession, monitor: &mut MySqlSession) -> TestResult {
    run(
        admin,
        &[
            "START TRANSACTION",
            "SELECT * FROM monitor_fixture WHERE id=1 FOR UPDATE",
        ],
    )
    .await?;
    let blocker = admin
        .admin()
        .ok_or("admin")?
        .own_process()
        .await?
        .to_string();
    let mut waiter = support::session().await?;
    let pid = waiter
        .admin()
        .ok_or("admin")?
        .own_process()
        .await?
        .to_string();
    let pending = tokio::spawn(async move {
        waiter
            .execute(
                "SELECT * FROM monitor_fixture WHERE id=1 FOR UPDATE /* test-only */",
                &mut Collect::default(),
            )
            .await
            .map(|_| ())
    });
    let result = observe(monitor, &pid, &blocker).await;
    run(admin, &["ROLLBACK"]).await?;
    pending.await??;
    let reading = result?;
    let mut store = LiveStore::default();
    store.ingest(Source::Activity, reading, 0, 0.0);
    let bytes = store.window(Some(Source::Activity), 1000, false)?;
    assert!(!String::from_utf8_lossy(&bytes).contains("test-only"));
    let batch = batch(bytes)?;
    let (pids, queries) = (strings(&batch, "pid")?, strings(&batch, "query")?);
    assert!(
        (0..batch.num_rows())
            .any(|i| pids.value(i) == pid
                && queries.value(i).to_lowercase().contains("monitor_fixture")),
        "active waiter query missing"
    );
    assert!(
        strings(&batch, "blocking_chain")?
            .iter()
            .flatten()
            .any(|chain| chain.contains(&pid) && chain.contains(&blocker))
    );
    Ok(())
}
async fn observe(monitor: &mut MySqlSession, pid: &str, blocker: &str) -> TestResult<Reading> {
    let start = Instant::now();
    let mut last = Vec::new();
    // InnoDB refreshes its INNODB_TRX/lock-wait cache only after 100 ms without
    // readers, so back-to-back polling would keep reading a stale snapshot.
    let mut ticks = tokio::time::interval(Duration::from_millis(250));
    while start.elapsed().as_secs() < 5 {
        ticks.tick().await;
        let sample = monitor.read(Source::Activity, 1_000).await;
        last.clone_from(&sample.rows);
        assert_eq!(sample.state, SampleState::Ok);
        if sample.rows.iter().any(|r| {
            r.first().and_then(Option::as_deref) == Some(pid)
                && r.get(1).and_then(Option::as_deref) == Some("active")
                && r.get(5)
                    .and_then(Option::as_deref)
                    .is_some_and(|ids| ids.split(',').any(|id| id == blocker))
        }) {
            return Ok(sample);
        }
    }
    Err(format!("active blocker/waiter pair {pid}<-{blocker} missing: {last:?}").into())
}
async fn counters(
    admin: &mut MySqlSession,
    monitor: &mut MySqlSession,
    instrumented: bool,
) -> TestResult {
    let mut store = LiveStore::default();
    store.ingest(
        Source::Database,
        monitor.read(Source::Database, 1_000).await,
        0,
        0.0,
    );
    run(
        admin,
        &[
            "SELECT value FROM monitor_fixture WHERE id=1",
            "UPDATE monitor_fixture SET value='test-only' WHERE id=1",
        ],
    )
    .await?;
    store.ingest(
        Source::Database,
        monitor.read(Source::Database, 1_000).await,
        1_000,
        1_000.0,
    );
    let batch = batch(store.window(None, 1_000, false)?)?;
    let names = strings(&batch, "metric")?;
    let values = column::<Float64Array>(&batch, "value")?;
    assert!(
        (0..batch.num_rows())
            .any(|i| names.value(i).ends_with(":queries") && values.value(i) > 0.0)
    );
    // MariaDB reports Innodb_deadlocks; MySQL needs the probed InnoDB metric.
    if instrumented || support::mariadb() {
        assert!(
            (0..batch.num_rows())
                .any(|i| names.value(i).ends_with(":deadlocks") && !values.is_null(i)),
            "deadlock rate missing"
        );
    }
    Ok(())
}
async fn restricted(admin: &mut MySqlSession, instrumented: bool) -> TestResult {
    run(
        admin,
        &[
            "DROP USER IF EXISTS 'monitor_low'@'%'",
            "CREATE USER 'monitor_low'@'%' IDENTIFIED BY 'pw'",
            "GRANT SELECT ON openrows_test.* TO 'monitor_low'@'%'",
            "GRANT SELECT ON performance_schema.* TO 'monitor_low'@'%'",
        ],
    )
    .await?;
    let mut request = support::request(SslMode::Prefer)?;
    request.user = "monitor_low".into();
    request.password = Some(openrows_net::SecretString::from("pw"));
    let mut low = connect(&request, &mut ConnectTrace::default()).await?;
    assert!(denied(
        state_of(&low.discover().await, Source::Activity),
        "PROCESS"
    ));
    let replication = if support::mariadb() {
        "SLAVE MONITOR"
    } else {
        "REPLICATION CLIENT"
    };
    let state = low.read(Source::Replication, 1_000).await.state;
    assert!(denied(Some(&state), replication), "{state:?}");
    Box::new(low).close().await?;
    run(admin, &["GRANT PROCESS ON *.* TO 'monitor_low'@'%'"]).await?;
    let mut low = connect(&request, &mut ConnectTrace::default()).await?;
    assert_eq!(
        state_of(&low.discover().await, Source::Activity),
        Some(&SampleState::Ok)
    );
    if instrumented {
        run(
            admin,
            &["REVOKE SELECT ON performance_schema.* FROM 'monitor_low'@'%'"],
        )
        .await?;
        let state = low.read(Source::Statements, 1_000).await.state;
        assert!(denied(Some(&state), "SELECT"), "{state:?}");
    }
    Box::new(low).close().await?;
    Ok(())
}
fn state_of(descriptors: &[Descriptor], source: Source) -> Option<&SampleState> {
    descriptors
        .iter()
        .find(|d| d.source == source)
        .map(|d| &d.state)
}
fn denied(state: Option<&SampleState>, grant: &str) -> bool {
    matches!(state, Some(SampleState::PermissionDenied(reason)) if reason.contains(grant))
}
fn strings<'a>(batch: &'a RecordBatch, name: &str) -> TestResult<&'a StringArray> {
    column(batch, name)
}
fn column<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &str) -> TestResult<&'a T> {
    batch
        .column_by_name(name)
        .ok_or(name)?
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| format!("{name} has an unexpected type").into())
}
fn batch(bytes: Vec<u8>) -> TestResult<RecordBatch> {
    StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")?
        .map_err(Into::into)
}
