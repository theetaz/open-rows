// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` binary `DataRow` frames → Arrow batches, from memory (no network).
//!
//! Timed: frame parsing, cell decoding, batch flushes and decoder construction.
//! Excluded: fixture generation and the per-iteration copy of the input frames.
use bytes::BytesMut;
use criterion::{BatchSize, Criterion, Throughput};
use openrows_driver_api::BatchPolicy;
use openrows_driver_postgres::ResultDecoder;
use openrows_driver_postgres::fixture::{self, Fixture, FixtureType};
use postgres_protocol::message::backend::Message;
use std::{collections::HashMap, hint::black_box};

struct Workload {
    name: &'static str,
    types: Vec<FixtureType>,
    rows: usize,
}

fn cycle(types: &[FixtureType], columns: usize) -> Vec<FixtureType> {
    types.iter().copied().cycle().take(columns).collect()
}

fn workloads() -> [Workload; 5] {
    use FixtureType::{Bool, Float8, Int4, Int8, Text};
    let mixed: Vec<_> = FixtureType::ALL
        .into_iter()
        .chain([Int4, Int8, Float8, Text, Bool])
        .collect();
    [
        Workload {
            name: "decode_pg_int",
            types: cycle(&[Int4, Int8], 10),
            rows: 100_000,
        },
        Workload {
            name: "decode_pg_text",
            types: vec![Text; 10],
            rows: 100_000,
        },
        Workload {
            name: "decode_pg_mixed",
            types: mixed.clone(),
            rows: 50_000,
        },
        Workload {
            name: "decode_pg_wide",
            types: cycle(&[Int4, Int8, Float8, Text], 200),
            rows: 5_000,
        },
        Workload {
            name: "decode_pg_first_batch",
            types: mixed,
            rows: 200,
        },
    ]
}

/// Decode every frame; returns the number of decoded rows.
fn decode(fixture: &Fixture, mut frames: BytesMut) -> usize {
    let mut decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), BatchPolicy::default());
    let mut rows = 0;
    while let Ok(Some(Message::DataRow(body))) = Message::parse(&mut frames) {
        match decoder.decode_row(body.buffer()) {
            Ok(Some(batch)) => rows += black_box(batch).num_rows(),
            Ok(None) => {}
            Err(_) => std::process::exit(1),
        }
    }
    match decoder.finish() {
        Ok(batch) => rows + batch.map_or(0, |b| black_box(b).num_rows()),
        Err(_) => std::process::exit(1),
    }
}

fn decode_benchmarks(criterion: &mut Criterion) {
    for workload in workloads() {
        let fixture = fixture::generate(&workload.types, workload.rows);
        let columns = workload.types.len();
        if decode(&fixture, fixture.frames.clone()) != workload.rows {
            std::process::exit(1);
        }
        let mut group = criterion.benchmark_group(workload.name);
        group.throughput(Throughput::Elements((columns * workload.rows) as u64));
        group.bench_function(format!("{columns}x{}", workload.rows), |bench| {
            bench.iter_batched(
                || fixture.frames.clone(),
                |frames| decode(&fixture, frames),
                BatchSize::LargeInput,
            );
        });
        group.finish();
    }
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    decode_benchmarks(&mut criterion);
    criterion.final_summary();
}
