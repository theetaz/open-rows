// SPDX-License-Identifier: Apache-2.0
//! Schema cache budgets at 50 000 objects (M1-16, NFR-PERF-08, docs/13 §4):
//!
//! - `schema_first_level_50k`: a relaunched cache serves one schema's
//!   50 000 tables from the persisted snapshot (store read + decode), the
//!   tree's first level on reconnect; budget ≤ 300 ms.
//! - `schema_search_50k`: warm name-index searches (exact, prefix,
//!   substring, subsequence, schema-filtered); budget ≤ 50 ms each.
//! - `schema_index_build_50k`: building the index (informational).
//!
//! Fixture generation, the snapshot write and the store open are untimed.
use criterion::{BatchSize, Criterion};
use openrows_driver_api::catalog::{ObjectKind, ObjectSummary};
use openrows_schema::{
    DatabaseKey, Listing, NameIndex, SchemaCache, Scope, SearchQuery, save_snapshot,
};
use openrows_store::LocalStore;
use std::hint::black_box;

const OBJECTS: usize = 50_000;
const WORDS: [&str; 8] = [
    "orders", "customer", "invoice", "event", "audit", "payment", "shipment", "product",
];

/// `OBJECTS` tables over ten schemas (`s0`…`s9`), or all in `public`.
fn objects(spread: bool) -> Vec<ObjectSummary> {
    (0..OBJECTS)
        .map(|i| ObjectSummary {
            schema: if spread {
                format!("s{}", i % 10)
            } else {
                "public".to_owned()
            },
            name: format!(
                "{}_{}_{i}",
                WORDS[i % WORDS.len()],
                WORDS[(i / 7) % WORDS.len()]
            ),
            kind: if i % 5 == 0 {
                ObjectKind::View
            } else {
                ObjectKind::Table
            },
            arguments: None,
            row_estimate: Some(i as u64),
            comment: (i % 3 == 0).then(|| format!("comment {i}")),
        })
        .collect()
}

type BenchResult = Result<(), Box<dyn std::error::Error>>;

fn first_level(criterion: &mut Criterion) -> BenchResult {
    let dir = tempfile::tempdir()?;
    let store = LocalStore::open(&dir.path().join("openrows.sqlite"))?;
    let key = DatabaseKey {
        profile_id: "bench".to_owned(),
        database: "app".to_owned(),
    };
    let scope = Scope::objects("public", &[ObjectKind::Table, ObjectKind::View]);
    let listing = Listing::Objects(objects(false));
    save_snapshot(&store, (&key, &scope), &listing, 1)?;
    let mut group = criterion.benchmark_group("schema_first_level_50k");
    group.bench_function("snapshot_restore", |bench| {
        bench.iter_batched(
            SchemaCache::default,
            |cache| {
                let cached = cache.get(&store, &key, &scope);
                let count = cached
                    .as_ref()
                    .and_then(|c| c.listing.objects())
                    .map_or(0, <[_]>::len);
                if count != OBJECTS {
                    std::process::exit(1);
                }
                black_box(cached)
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
    Ok(())
}

fn search(criterion: &mut Criterion) -> BenchResult {
    let index = NameIndex::new(objects(true));
    let queries = [
        ("exact", "orders_audit_4", None),
        ("prefix", "invoice_ev", None),
        ("substring", "ment_pro", None),
        ("subsequence", "cstmrpymt", None),
        ("schema_filter", "pay", Some("s3")),
    ];
    let mut group = criterion.benchmark_group("schema_search_50k");
    for (name, text, schema) in queries {
        let query = SearchQuery {
            text,
            schema,
            limit: 50,
        };
        if index.search(&query).total == 0 {
            return Err(format!("{name} finds nothing").into());
        }
        group.bench_function(name, |bench| {
            bench.iter(|| black_box(index.search(black_box(&query))).total);
        });
    }
    group.finish();
    let mut group = criterion.benchmark_group("schema_index_build_50k");
    let source = objects(true);
    group.bench_function("build", |bench| {
        bench.iter_batched(|| source.clone(), NameIndex::new, BatchSize::LargeInput);
    });
    group.finish();
    Ok(())
}

fn main() -> BenchResult {
    let mut criterion = Criterion::default().configure_from_args();
    first_level(&mut criterion)?;
    search(&mut criterion)?;
    criterion.final_summary();
    Ok(())
}
