// SPDX-License-Identifier: Apache-2.0
//! Local store: migrations, WAL, downgrade protection and history.

use openrows_store::{
    ChangeSetRecord, HistoryRecord, HistorySource, HistoryStatus, LocalStore, SCHEMA_VERSION,
    StoreError,
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
    assert_eq!(store.meta("schema_version").unwrap().as_deref(), Some("3"));
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

    let entries = store.search_history(Some("p1"), "", 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].record, failed);
    assert_eq!(entries[1].record.started_at_ms, 1_000);
    assert_eq!(store.search_history(Some("p1"), "", 1).unwrap().len(), 1);
    assert_eq!(store.search_history(None, "", 10).unwrap().len(), 3);
}

#[test]
fn history_search_matches_words_in_one_profile_and_deletes_keep_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let mut orders = record("p1", 1_000, HistoryStatus::Ok);
    orders.statement = "SELECT * FROM public.orders WHERE total > 10".to_owned();
    let orders_id = store.record_history(&orders).unwrap();
    let mut users = record("p1", 2_000, HistoryStatus::Ok);
    users.statement = "SELECT name FROM users".to_owned();
    store.record_history(&users).unwrap();
    store
        .record_history(&HistoryRecord {
            statement: "SELECT total FROM orders".to_owned(),
            ..record("p2", 3_000, HistoryStatus::Ok)
        })
        .unwrap();

    let found = store.search_history(Some("p1"), "ORDERS  tot", 10).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].record.statement, orders.statement);
    assert_eq!(store.search_history(None, "orders", 10).unwrap().len(), 2);
    assert!(
        store
            .search_history(Some("p1"), "\"unbalanced", 10)
            .unwrap()
            .is_empty()
    );

    assert!(store.delete_history(orders_id).unwrap());
    assert!(!store.delete_history(orders_id).unwrap());
    assert!(
        store
            .search_history(Some("p1"), "orders", 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.clear_history("p1").unwrap(), 1);
    assert!(store.search_history(Some("p1"), "", 10).unwrap().is_empty());
    assert_eq!(store.search_history(None, "orders", 10).unwrap().len(), 1);
}

#[test]
fn change_sets_are_journaled_per_table_until_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let mut record = ChangeSetRecord {
        id: "p1/orders/public/users".to_owned(),
        profile_id: "p1".to_owned(),
        target: r#"{"table":"users"}"#.to_owned(),
        ops: r#"[{"kind":"update"}]"#.to_owned(),
        updated_at_ms: 1_000,
    };
    assert!(store.change_set(&record.id).unwrap().is_none());
    store.save_change_set(&record).unwrap();
    assert_eq!(
        store.change_set(&record.id).unwrap().as_ref(),
        Some(&record)
    );
    record.ops = r#"[{"kind":"update"},{"kind":"delete"}]"#.to_owned();
    record.updated_at_ms = 2_000;
    store.save_change_set(&record).unwrap();
    assert_eq!(
        store.change_set(&record.id).unwrap().as_ref(),
        Some(&record)
    );
    store.clear_change_set(&record.id).unwrap();
    store.clear_change_set(&record.id).unwrap();
    assert!(store.change_set(&record.id).unwrap().is_none());
}
