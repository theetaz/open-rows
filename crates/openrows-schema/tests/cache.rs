// SPDX-License-Identifier: Apache-2.0
//! Schema cache freshness, snapshots across a relaunch, invalidation races
//! and eviction, without a database server.

use openrows_driver_api::catalog::{ObjectKind, ObjectSummary, SchemaSummary};
use openrows_schema::{DatabaseKey, Listing, SchemaCache, Scope, save_snapshot};
use openrows_store::LocalStore;

fn key(profile: &str) -> DatabaseKey {
    DatabaseKey {
        profile_id: profile.to_owned(),
        database: "orders".to_owned(),
    }
}

fn schemas(names: &[&str]) -> Listing {
    Listing::Schemas(
        names
            .iter()
            .map(|name| SchemaSummary {
                name: (*name).to_owned(),
                system: false,
                comment: None,
            })
            .collect(),
    )
}

fn names(listing: &Listing) -> Vec<String> {
    listing
        .schemas()
        .unwrap_or_default()
        .iter()
        .map(|schema| schema.name.clone())
        .collect()
}

#[test]
fn live_reads_are_fresh_and_snapshots_come_back_stale_after_a_relaunch() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let cache = SchemaCache::default();
    let (key, scope) = (key("p1"), Scope::Schemas);
    assert!(cache.get(&store, &key, &scope).is_none());
    let token = cache.begin(&key);
    let cached = cache.put(&key, &scope, schemas(&["public", "sales"]), token);
    assert!(!cached.stale);
    save_snapshot(&store, (&key, &scope), &cached.listing, 1).unwrap();
    assert!(!cache.get(&store, &key, &scope).unwrap().stale);

    let relaunched = SchemaCache::default();
    let restored = relaunched.get(&store, &key, &scope).unwrap();
    assert!(restored.stale);
    assert_eq!(names(&restored.listing), ["public", "sales"]);
}

#[test]
fn invalidation_marks_the_connection_stale_and_overlapping_reads_stay_stale() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let cache = SchemaCache::default();
    let (p1, p2, scope) = (key("p1"), key("p2"), Scope::Schemas);
    for key in [&p1, &p2] {
        let token = cache.begin(key);
        cache.put(key, &scope, schemas(&["public"]), token);
    }
    let racing = cache.begin(&p1);
    cache.invalidate("p1");
    assert!(cache.get(&store, &p1, &scope).unwrap().stale);
    assert!(!cache.get(&store, &p2, &scope).unwrap().stale);
    assert!(cache.put(&p1, &scope, schemas(&["old"]), racing).stale);
    let token = cache.begin(&p1);
    assert!(!cache.put(&p1, &scope, schemas(&["new"]), token).stale);
}

#[test]
fn snapshots_never_replace_a_live_read_and_are_forgotten_with_the_profile() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let (key, scope) = (key("p1"), Scope::Schemas);
    save_snapshot(&store, (&key, &scope), &schemas(&["snapshot"]), 1).unwrap();
    let cache = SchemaCache::default();
    let token = cache.begin(&key);
    cache.put(&key, &scope, schemas(&["live"]), token);
    let cached = cache.get(&store, &key, &scope).unwrap();
    assert_eq!(
        (names(&cached.listing), cached.stale),
        (vec!["live".to_owned()], false)
    );
    cache.forget(&store, "p1").unwrap();
    assert!(cache.get(&store, &key, &scope).is_none());
}

#[test]
fn object_scopes_and_the_index_share_one_database_entry() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalStore::open(&dir.path().join("openrows.sqlite")).unwrap();
    let cache = SchemaCache::default();
    let key = key("p1");
    let orders = ObjectSummary {
        schema: "public".to_owned(),
        name: "orders".to_owned(),
        kind: ObjectKind::Table,
        arguments: None,
        row_estimate: Some(10),
        comment: None,
    };
    let tables = Scope::objects("public", &[ObjectKind::Table]);
    let token = cache.begin(&key);
    cache.put(&key, &tables, Listing::Objects(vec![orders.clone()]), token);
    let index = openrows_schema::NameIndex::new(vec![orders.clone()]);
    cache.put(&key, &Scope::Index, Listing::Index(index), token);
    let hit = cache.get(&store, &key, &Scope::Index).unwrap();
    assert_eq!(hit.listing.objects(), Some(&[orders][..]));
    assert!(
        cache
            .get(&store, &key, &Scope::objects("public", &[ObjectKind::View]))
            .is_none()
    );
}

#[test]
fn a_changed_fingerprint_is_reported_once_and_invalidation_resets_it() {
    let cache = SchemaCache::default();
    let orders = key("p");
    assert!(!cache.observe_fingerprint(&orders, "a".to_owned()));
    assert!(!cache.observe_fingerprint(&orders, "a".to_owned()));
    assert!(cache.observe_fingerprint(&orders, "b".to_owned()));
    assert!(!cache.observe_fingerprint(&orders, "b".to_owned()));
    // DDL from the app already invalidated: the next fingerprint is the baseline.
    cache.invalidate("p");
    assert!(!cache.observe_fingerprint(&orders, "c".to_owned()));
    assert!(!cache.observe_fingerprint(&key("other"), "z".to_owned()));
}
