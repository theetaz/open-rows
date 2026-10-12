// SPDX-License-Identifier: Apache-2.0
//! `MySQL` buffer requests include physical reads; this differs from `PostgreSQL` blocks.
use arrow_array::{Array, Float64Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_monitor::{Column, LiveStore, MetricKind, Reading, SampleState, Source};
use std::io::Cursor;
fn sample(requests: u64, reads: u64, epoch: &str) -> Reading {
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.into(),
        partial: false,
        columns: ["id", "buffer_read_requests", "buffer_reads"]
            .iter()
            .enumerate()
            .map(|(i, name)| Column {
                name: (*name).into(),
                unit: "count".into(),
                kind: if i == 0 {
                    MetricKind::Text
                } else {
                    MetricKind::Counter
                },
            })
            .collect(),
        rows: vec![vec![
            Some("server".into()),
            Some(requests.to_string()),
            Some(reads.to_string()),
        ]],
    }
}
#[test]
fn buffer_pool_ratio_uses_interval_requests_and_never_fabricates_zero()
-> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    for (i, requests, reads, epoch) in [
        (0, 100, 10, "a"),
        (1, 200, 20, "a"),
        (2, 200, 20, "a"),
        (3, 500, 40, "b"),
    ] {
        store.ingest(
            Source::Database,
            sample(requests, reads, epoch),
            i * 1000,
            f64::from(u32::try_from(i)?) * 1000.0,
        );
    }
    let batch = StreamReader::try_new(Cursor::new(store.window(None, 1000, false)?), None)?
        .next()
        .ok_or("batch")??;
    let names = batch
        .column_by_name("metric")
        .ok_or("metric")?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("names")?;
    let values = batch
        .column_by_name("value")
        .ok_or("value")?
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("values")?;
    let units = batch
        .column_by_name("unit")
        .ok_or("unit")?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("units")?;
    let indices: Vec<_> = (0..batch.num_rows())
        .filter(|i| names.value(*i) == "Database:buffer_pool_hit_ratio")
        .collect();
    assert_eq!(indices.len(), 4);
    assert!(indices.iter().all(|i| units.value(*i) == "fraction"));
    assert!(values.is_null(indices[0]));
    assert!((values.value(indices[1]) - 0.9).abs() < f64::EPSILON);
    assert!(values.is_null(indices[2]));
    assert!(values.is_null(indices[3]));
    Ok(())
}
