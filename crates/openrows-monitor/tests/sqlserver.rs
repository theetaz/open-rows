// SPDX-License-Identifier: Apache-2.0
//! SQL Server reports its buffer cache hit ratio directly; no derived ratio is invented.
use arrow_array::{Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_monitor::{Column, LiveStore, MetricKind, Reading, SampleState, Source};
use std::io::Cursor;

fn column(name: &str, unit: &str, kind: MetricKind) -> Column {
    Column {
        name: name.into(),
        unit: unit.into(),
        kind,
    }
}

#[test]
fn direct_ratio_gauges_and_counter_rates_without_derived_cache_ratio()
-> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    for (i, requests, ratio) in [(0_u32, 100, "0.99"), (1, 160, "0.98")] {
        let reading = Reading {
            terminal: false,
            state: SampleState::Ok,
            epoch: "2026-10-12T10:00:00".into(),
            partial: false,
            columns: vec![
                column("server_id", "text", MetricKind::Text),
                column("batch_requests", "count", MetricKind::Counter),
                column("buffer_cache_hit_ratio", "fraction", MetricKind::Gauge),
            ],
            rows: vec![vec![
                Some("server".into()),
                Some(requests.to_string()),
                Some(ratio.into()),
            ]],
        };
        store.ingest(
            Source::Database,
            reading,
            u64::from(i) * 1_000,
            f64::from(i) * 1_000.0,
        );
    }
    let batch = StreamReader::try_new(Cursor::new(store.window(None, 1000, false)?), None)?
        .next()
        .ok_or("batch")??;
    let metrics = batch
        .column_by_name("metric")
        .ok_or("metric")?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("names")?;
    let names: Vec<_> = (0..metrics.len()).map(|i| metrics.value(i)).collect();
    assert!(names.iter().any(|n| n.ends_with(":buffer_cache_hit_ratio")));
    assert!(names.iter().any(|n| n.ends_with(":batch_requests")));
    assert!(!names.contains(&"Database:cache_hit_ratio"));
    assert!(!names.contains(&"Database:buffer_pool_hit_ratio"));
    Ok(())
}
