// SPDX-License-Identifier: Apache-2.0
//! Heap-allocation proof for the primitive decode path (docs/13 §3.1).
//!
//! Its own test binary because it installs a counting global allocator.
use dhat::{HeapStats, Profiler};
use openrows_driver_api::{BatchPolicy, DriverError};
use openrows_driver_postgres::ResultDecoder;
use openrows_driver_postgres::fixture::{self, FixtureType};
use postgres_protocol::message::backend::Message;
use std::collections::HashMap;

const WARM_UP_ROWS: usize = 7;

#[global_allocator]
static ALLOCATOR: dhat::Alloc = dhat::Alloc;

const PRIMITIVES: [FixtureType; 11] = [
    FixtureType::Int2,
    FixtureType::Int4,
    FixtureType::Int8,
    FixtureType::Float4,
    FixtureType::Float8,
    FixtureType::Bool,
    FixtureType::Timestamp,
    FixtureType::TimestampTz,
    FixtureType::Date,
    FixtureType::Uuid,
    FixtureType::Decimal,
];

/// Parse the next `DataRow` frame and decode it; true when a batch was flushed.
fn decode_next(
    decoder: &mut ResultDecoder,
    frames: &mut bytes::BytesMut,
) -> Result<bool, DriverError> {
    match Message::parse(frames)? {
        Some(Message::DataRow(body)) => Ok(decoder.decode_row(body.buffer())?.is_some()),
        _ => Err(DriverError::Protocol("fixture ended early".to_owned())),
    }
}

#[test]
fn primitive_cells_decode_without_heap_allocation() -> Result<(), DriverError> {
    let _profiler = Profiler::builder().testing().build();
    let rows = 5_000;
    let fixture = fixture::generate(&PRIMITIVES, rows);
    let policy = BatchPolicy {
        first_batch_rows: rows,
        target_cells: rows * PRIMITIVES.len(),
        max_bytes: usize::MAX,
    };
    let mut decoder = ResultDecoder::new(&fixture.columns, &HashMap::new(), policy);
    let mut frames = fixture.frames.clone();
    // Per-batch, not per-cell, allocations happen in the first 7 rows: the first
    // frame split makes the read buffer shared, and Arrow materialises each
    // column's validity bitmap on its first NULL (the fixture has one per 7 rows).
    for _ in 0..WARM_UP_ROWS {
        assert!(!decode_next(&mut decoder, &mut frames)?);
    }
    let before = HeapStats::get();
    for _ in WARM_UP_ROWS..rows - 1 {
        assert!(!decode_next(&mut decoder, &mut frames)?);
    }
    let after = HeapStats::get();
    let cells = (rows - WARM_UP_ROWS - 1) * PRIMITIVES.len();
    assert_eq!(
        after.total_blocks - before.total_blocks,
        0,
        "{cells} primitive cells must decode without allocating"
    );
    assert!(
        decode_next(&mut decoder, &mut frames)?,
        "last row flushes the batch"
    );
    Ok(())
}
