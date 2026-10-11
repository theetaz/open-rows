// SPDX-License-Identifier: Apache-2.0
//! Dashboard gauges distinguish observed empty sources from missing sources.
use arrow_array::{Array, Float64Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_monitor::{Column, LiveStore, MetricKind, Reading, SampleState, Source};
use std::io::Cursor;

#[test]
fn known_empty_activity_counts_are_real_zeros_but_permission_loss_is_a_gap()
-> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    store.ingest(
        Source::Activity,
        Reading {
            terminal: false,
            state: SampleState::Ok,
            epoch: "test".into(),
            partial: false,
            columns: vec![],
            rows: vec![],
        },
        0,
        0.0,
    );
    let bytes = store.window(None, 1000, false)?;
    let first = StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let values = first
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("values")?;
    assert!(values.iter().all(|value| value == Some(0.0)));
    assert!(values.len() >= 5);
    store.ingest(
        Source::Activity,
        Reading::missing(SampleState::PermissionDenied("pg_monitor".into())),
        1000,
        1000.0,
    );
    let next = StreamReader::try_new(Cursor::new(store.window(None, 1000, false)?), None)?
        .next()
        .ok_or("batch")??;
    let values = next
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("values")?;
    assert!(values.iter().any(|value| value.is_none()));
    Ok(())
}
#[test]
fn replication_null_lag_and_no_replicas_stay_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    store.ingest(
        Source::Replication,
        Reading {
            terminal: false,
            state: SampleState::Ok,
            epoch: "test".into(),
            partial: false,
            columns: vec![Column {
                name: "replay_lag_ms".into(),
                unit: "ms".into(),
                kind: MetricKind::Gauge,
            }],
            rows: vec![vec![None]],
        },
        0,
        0.0,
    );
    let batch = StreamReader::try_new(Cursor::new(store.window(None, 1000, false)?), None)?
        .next()
        .ok_or("batch")??;
    assert!(
        batch
            .column(2)
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or("values")?
            .iter()
            .all(|value| value.is_none())
    );
    let states = batch
        .column(3)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("states")?;
    assert!(states.iter().all(|state| state == Some("unknown")));
    Ok(())
}
