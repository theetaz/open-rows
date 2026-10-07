// SPDX-License-Identifier: Apache-2.0
//! Local store: migrations, WAL, downgrade protection and history.

use openrows_store::{
    HistoryRecord, HistorySource, HistoryStatus, LocalStore, SCHEMA_VERSION, StoreError,
};

fn record(profile_id: &str, started_at_ms: i64, status: HistoryStatus) -> HistoryRecord {
    HistoryRecord {
        profile_id: profile_id.to_owned(),
        database: Some("orders".to_owned()),
        started_at_ms,
        duration_ms: Some(12),
        status,
        rows: Some(3),
        statement: "SELECT 1".to_owned(),
        error_message: None,
        source: HistorySource::Editor,
    }
}

#[test]
fn fresh_database_is_migrated_in_wal_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("openrows.sqlite");
    let store = LocalStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.meta("schema_version").unwrap().as_deref(), Some("1"));
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    assert!(
        !dir.path().join("backups").exists(),
        "fresh DB needs no backup"
    );
}

#[test]
fn reopening_is_idempotent_and_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("openrows.sqlite");
    let store = LocalStore::open(&path).unwrap();
    store.set_meta("theme", "dark").unwrap();
    drop(store);
    let store = LocalStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.meta("theme").unwrap().as_deref(), Some("dark"));
    assert!(store.meta("missing").unwrap().is_none());
}

#[test]
fn newer_schema_is_refused_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("openrows.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    let error = LocalStore::open(&path).unwrap_err();
    assert!(matches!(
        error,
        StoreError::NewerSchema {
            found: 99,
            supported: SCHEMA_VERSION
        }
    ));
}

#[test]
fn history_round_trips_newest_first_per_profile() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    store
        .record_history(&record("p1", 1_000, HistoryStatus::Ok))
        .unwrap();
    let mut failed = record("p1", 2_000, HistoryStatus::Error);
    failed.error_message = Some("syntax error".to_owned());
    failed.source = HistorySource::EditCommit;
    store.record_history(&failed).unwrap();
    store
        .record_history(&record("p2", 3_000, HistoryStatus::Cancelled))
        .unwrap();

    let entries = store.recent_history("p1", 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].record, failed);
    assert_eq!(entries[1].record.started_at_ms, 1_000);
    assert_eq!(store.recent_history("p1", 1).unwrap().len(), 1);
}
