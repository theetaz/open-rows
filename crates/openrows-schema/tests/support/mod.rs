// SPDX-License-Identifier: Apache-2.0
//! The engine-independent cache happy path, run on a real connection by
//! `sqlite.rs`, `postgres.rs` and `mysql.rs`.

use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Connection, catalog, editor};
use openrows_schema::{
    Cached, DatabaseKey, Listing, NameIndex, SchemaCache, Scope, SearchQuery, save_snapshot,
};
use openrows_store::LocalStore;

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Where the test table lives: `(database, schema, table)`.
pub struct Place<'a> {
    pub database: &'a str,
    pub schema: &'a str,
    pub table: &'a str,
}

/// Read `scope` live on `connection` into the cache and persist it.
async fn read_live(
    cache: &SchemaCache,
    store: &LocalStore,
    (key, scope): (&DatabaseKey, &Scope),
    connection: &mut dyn Connection,
) -> TestResult<Cached> {
    let token = cache.begin(key);
    let catalog = catalog(connection)?;
    let listing = match scope {
        Scope::Databases => Listing::Databases(catalog.list_databases().await?),
        Scope::Schemas => Listing::Schemas(catalog.list_schemas().await?),
        Scope::Objects { schema, kinds } => {
            Listing::Objects(catalog.list_objects(schema, kinds).await?)
        }
        Scope::Index => Listing::Index(NameIndex::new(catalog.search_index().await?)),
    };
    let cached = cache.put(key, scope, listing, token);
    save_snapshot(store, (key, scope), &cached.listing, 1)?;
    Ok(cached)
}

fn tables(cached: &Cached) -> Vec<String> {
    cached
        .listing
        .objects()
        .unwrap_or_default()
        .iter()
        .map(|object| object.name.clone())
        .collect()
}

/// Lists, snapshots and finds `place.table`, restores it after a simulated
/// relaunch, then drops it and sees the invalidated listing refresh.
pub async fn happy_path(connection: &mut dyn Connection, place: &Place<'_>) -> TestResult {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("openrows.sqlite"))?;
    let cache = SchemaCache::default();
    let key = DatabaseKey {
        profile_id: "profile".to_owned(),
        database: place.database.to_owned(),
    };
    let section = Scope::objects(place.schema, &[ObjectKind::Table]);
    let schemas = read_live(&cache, &store, (&key, &Scope::Schemas), connection).await?;
    let listed = schemas.listing.schemas().unwrap_or_default();
    assert!(listed.iter().any(|schema| schema.name == place.schema));
    let objects = read_live(&cache, &store, (&key, &section), connection).await?;
    assert!(tables(&objects).contains(&place.table.to_owned()));
    read_live(&cache, &store, (&key, &Scope::Index), connection).await?;

    let relaunched = SchemaCache::default();
    let index = relaunched
        .get(&store, &key, &Scope::Index)
        .ok_or("the index snapshot was not restored")?;
    assert!(index.stale);
    let query = SearchQuery {
        text: &place.table[..4],
        schema: Some(place.schema),
        limit: 10,
    };
    let hits = index.listing.index().ok_or("not an index")?.search(&query);
    assert_eq!(
        hits.objects.first().map(|object| object.name.as_str()),
        Some(place.table)
    );

    let before = catalog(connection)?
        .schema_fingerprint()
        .await?
        .ok_or("the engine has no fingerprint")?;
    assert!(!relaunched.observe_fingerprint(&key, before.clone()));
    assert!(!relaunched.observe_fingerprint(&key, before.clone()));

    let drop_table = format!("DROP TABLE {}", place.table);
    editor(connection)?.commit_ddl(&[drop_table]).await?;
    let after = catalog(connection)?.schema_fingerprint().await?;
    let after = after.ok_or("the engine has no fingerprint")?;
    assert_ne!(before, after, "DDL changes the fingerprint");
    assert!(relaunched.observe_fingerprint(&key, after));
    relaunched.invalidate("profile");
    let refreshed = read_live(&relaunched, &store, (&key, &section), connection).await?;
    assert!(!refreshed.stale);
    assert!(!tables(&refreshed).contains(&place.table.to_owned()));
    Ok(())
}
