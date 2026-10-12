// SPDX-License-Identifier: Apache-2.0
//! Monitor reads share the one hosted `DuckDB` session with user statements.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use fixture::*;
use openrows_core::{FacetMonitor, QueryEvent, QueryHandle, SessionManager};
use openrows_driver_api::{FileSource, Session};
use openrows_monitor::{MonitorReader, Reading, SampleState, Source};
use std::sync::Arc;
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Roughly 10^11 rows: runs until cancelled.
const LONG: &str = "SELECT sum(i % 7) FROM range(100000000000) t(i)";

fn cell(reading: &Reading, name: &str) -> Option<String> {
    let at = reading.columns.iter().position(|c| c.name == name)?;
    reading.rows.first()?.get(at)?.clone()
}

async fn finished(mut query: QueryHandle) -> Vec<QueryEvent> {
    let mut events = Vec::new();
    while let Some(event) = query.next_event().await {
        events.push(event);
    }
    events
}

type Monitored = (SessionManager, openrows_types::SessionId, FacetMonitor);

async fn monitored(source: FileSource) -> Result<Monitored, Box<dyn std::error::Error>> {
    let connection = open(source).await?;
    let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
    let session = manager
        .open(async move { Ok(Box::new(connection) as Box<dyn Session>) })
        .await?;
    let port = manager.facet_port(session)?;
    let monitor = FacetMonitor::new(port, openrows_host_client::duckdb_monitor_descriptors());
    Ok((manager, session, monitor))
}

#[tokio::test]
async fn samples_queue_behind_user_statements_and_cancel_restores_them() -> TestResult {
    let root = tempfile::tempdir_in(".")?;
    let path = std::fs::canonicalize(root.path())?.join("monitor.duckdb");
    let (manager, session, mut monitor) = monitored(FileSource::Disk {
        path,
        read_only: false,
        create: true,
    })
    .await?;
    finished(manager.execute(
        session,
        "CREATE TABLE t AS SELECT range AS id FROM range(1000)",
    )?)
    .await;
    let idle = monitor.read(Source::Database, 1_000).await;
    assert_eq!(idle.state, SampleState::Ok, "{idle:?}");
    assert!(
        cell(&idle, "block_size").is_some_and(|v| v != "0"),
        "{idle:?}"
    );
    assert!(cell(&idle, "database_bytes").is_some(), "{idle:?}");
    assert!(cell(&idle, "memory_limit_bytes").is_some(), "{idle:?}");
    assert!(cell(&idle, "threads").is_some(), "{idle:?}");
    assert_eq!(cell(&idle, "requests").as_deref(), Some("1"));

    let long = manager.execute(session, LONG)?;
    let cancel = long.canceller();
    let started = Instant::now();
    let busy = monitor.read(Source::Database, 400).await;
    assert_eq!(busy.state, SampleState::Gap, "{busy:?}");
    assert!(!busy.terminal);
    assert!(started.elapsed() < Duration::from_millis(390));
    cancel.cancel();
    let events = tokio::time::timeout(Duration::from_secs(10), finished(long)).await?;
    assert!(
        events.iter().any(|e| matches!(e, QueryEvent::Cancelled)),
        "{events:?}"
    );

    let after = monitor.read(Source::Database, 1_000).await;
    assert_eq!(after.state, SampleState::Ok, "{after:?}");
    let statements = monitor.read(Source::Statements, 1_000).await;
    assert_eq!(statements.rows.len(), 2, "{statements:?}");
    let sessions = monitor.read(Source::Activity, 1_000).await;
    assert!(matches!(sessions.state, SampleState::Unavailable(_)));
    manager.close(session).await?;
    Ok(())
}

#[tokio::test]
async fn host_loss_is_a_terminal_typed_state() -> TestResult {
    let (manager, session, mut monitor) = monitored(memory("monitor-lost")).await?;
    let events = finished(manager.execute(session, "OPENROWS_TEST_ABORT")?).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, QueryEvent::Failed { .. })),
        "{events:?}"
    );
    let lost = monitor.read(Source::Database, 1_000).await;
    assert!(lost.terminal, "{lost:?}");
    assert!(
        matches!(&lost.state, SampleState::Unavailable(reason) if reason.contains("owning session")),
        "{lost:?}"
    );
    Ok(())
}
