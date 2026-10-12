// SPDX-License-Identifier: Apache-2.0
//! A dedicated monitor connection sees this app's own statements against the
//! file and cheap file/WAL diagnostics, never its own probes.
use openrows_driver_api::{Connection, DiscardSink, Session};
use openrows_driver_sqlite::open;
use openrows_monitor::{Reading, SampleState, Source};

fn cell<'a>(reading: &'a Reading, name: &str) -> Option<&'a str> {
    let index = reading.columns.iter().position(|c| c.name == name)?;
    reading.rows.first()?.get(index)?.as_deref()
}

fn number(reading: &Reading, name: &str) -> f64 {
    cell(reading, name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(f64::NAN)
}

#[tokio::test]
async fn monitor_reads_own_statements_and_wal_file_state() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("app.db");
    let mut user = open(&path, false, true)?;
    for sql in [
        "PRAGMA journal_mode = WAL",
        "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)",
        "INSERT INTO t (name) VALUES ('alice'), ('bob'), ('carol')",
        "SELECT * FROM t WHERE name = 'alice'",
        "SELECT * FROM t WHERE name = 'bob'",
    ] {
        user.execute(sql, &mut DiscardSink).await?;
    }
    let mut monitor = open(&path, false, false)?;
    let reader = monitor.monitor().ok_or("monitor reader")?;
    let descriptors = reader.discover().await;
    let state = |source| {
        descriptors
            .iter()
            .find(|d| d.source == source)
            .map(|d| d.state.clone())
    };
    assert_eq!(state(Source::Database), Some(SampleState::Ok));
    assert_eq!(state(Source::Statements), Some(SampleState::Ok));
    for source in [Source::Activity, Source::Replication, Source::Io] {
        assert!(
            matches!(state(source), Some(SampleState::Unavailable(reason)) if reason.contains("Own OpenRows sessions only")),
            "{source:?}"
        );
    }

    let file = reader.read(Source::Database, 1_000).await;
    assert_eq!(file.state, SampleState::Ok);
    assert_eq!(cell(&file, "journal_mode"), Some("wal"));
    assert!(number(&file, "page_size") >= 512.0);
    assert!(number(&file, "page_count") >= 2.0);
    assert!(number(&file, "freelist_count") >= 0.0);
    assert!(number(&file, "file_bytes") > 0.0);
    assert!(number(&file, "wal_bytes") > 0.0);
    assert!(number(&file, "wal_frames") > 0.0);
    assert!(number(&file, "checkpoint_backlog") > 0.0);
    assert_eq!(cell(&file, "requests"), Some("5"));

    let statements = reader.read(Source::Statements, 1_000).await;
    let queries: Vec<_> = statements
        .rows
        .iter()
        .filter_map(|row| row.last().cloned().flatten())
        .collect();
    assert!(
        queries
            .iter()
            .any(|q| q.starts_with("SELECT * FROM t WHERE name = ?"))
    );
    assert!(queries.iter().all(|q| !q.contains("alice")));
    let selects = statements
        .rows
        .iter()
        .find(|row| {
            row.last()
                .and_then(Option::as_deref)
                .is_some_and(|q| q.starts_with("SELECT"))
        })
        .ok_or("select row")?;
    assert_eq!(selects.get(1).cloned().flatten().as_deref(), Some("2"));
    assert_eq!(
        statements.rows.len(),
        4,
        "monitor probes are never recorded"
    );
    Ok(())
}

#[tokio::test]
async fn rollback_journal_files_leave_wal_state_unknown() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("plain.db");
    let mut user = open(&path, false, true)?;
    user.execute("CREATE TABLE t (a)", &mut DiscardSink).await?;
    let mut monitor = open(&path, true, false)?;
    let reader = monitor.monitor().ok_or("monitor reader")?;
    let file = reader.read(Source::Database, 1_000).await;
    assert_eq!(file.state, SampleState::Ok);
    assert_eq!(cell(&file, "journal_mode"), Some("delete"));
    for name in ["wal_bytes", "wal_frames", "checkpoint_backlog"] {
        assert_eq!(cell(&file, name), None, "{name}");
    }
    Ok(())
}
