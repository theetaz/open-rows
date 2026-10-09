// SPDX-License-Identifier: Apache-2.0
//! Schema-cache snapshots: upsert per scope, per-database reads, eviction.

use openrows_store::{LocalStore, SchemaSnapshotRecord};

fn snapshot(profile: &str, database: &str, scope: &str, at: i64) -> SchemaSnapshotRecord {
    SchemaSnapshotRecord {
        profile_id: profile.to_owned(),
        database: database.to_owned(),
        scope: scope.to_owned(),
        payload: format!("{scope}@{at}").into_bytes(),
        fetched_at_ms: at,
    }
}

#[test]
fn snapshots_upsert_per_scope_and_read_per_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    for (database, scope, at) in [
        ("orders", "schemas", 1),
        ("orders", "schemas", 2),
        ("orders", "index", 3),
        ("billing", "schemas", 4),
    ] {
        store
            .save_schema_snapshot(&snapshot("p1", database, scope, at))
            .unwrap();
    }
    let mut rows = store.schema_snapshots("p1", "orders").unwrap();
    rows.sort_by(|a, b| a.scope.cmp(&b.scope));
    assert_eq!(
        rows,
        vec![
            snapshot("p1", "orders", "index", 3),
            snapshot("p1", "orders", "schemas", 2)
        ]
    );
    assert!(store.schema_snapshots("p2", "orders").unwrap().is_empty());
}

#[test]
fn snapshots_are_evicted_with_the_profile_or_when_unused() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    for (profile, at) in [("p1", 10), ("p2", 20), ("p3", 30)] {
        store
            .save_schema_snapshot(&snapshot(profile, "main", "schemas", at))
            .unwrap();
    }
    assert_eq!(store.delete_schema_snapshots("p1").unwrap(), 1);
    assert_eq!(store.prune_schema_snapshots(25).unwrap(), 1);
    assert!(store.schema_snapshots("p2", "main").unwrap().is_empty());
    assert_eq!(store.schema_snapshots("p3", "main").unwrap().len(), 1);
}
