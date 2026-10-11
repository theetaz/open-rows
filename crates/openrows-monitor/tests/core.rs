// SPDX-License-Identifier: Apache-2.0
//! Core budgets and missing-data semantics, independent of a database or Tauri.
use arrow_array::{Array, Float64Array, StringArray};
use arrow_ipc::reader::StreamReader;
use openrows_monitor::{
    Cadence, Column, Counter, LiveStore, MetricKind, Reading, SampleState, Source, counter_delta,
    ratio,
};
use std::io::Cursor;

fn reading(value: &str, epoch: &str) -> Reading {
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.into(),
        partial: false,
        columns: vec![
            column("id", MetricKind::Text),
            column("calls", MetricKind::Counter),
        ],
        rows: vec![vec![Some("db".into()), Some(value.into())]],
    }
}
fn column(name: &str, kind: MetricKind) -> Column {
    Column {
        name: name.into(),
        unit: "count".into(),
        kind,
    }
}
fn batch(bytes: Vec<u8>) -> Result<arrow_array::RecordBatch, Box<dyn std::error::Error>> {
    StreamReader::try_new(Cursor::new(bytes), None)?
        .next()
        .ok_or("no batch")?
        .map_err(Into::into)
}

#[test]
fn delta_baseline_reset_rollover_and_zero_denominator_are_explicit() {
    let mut current = Counter {
        value: u64::MAX - 1,
        epoch: "server/reset".into(),
        elapsed_ms: 1_000,
    };
    assert_eq!(counter_delta(None, &current), (None, SampleState::Unknown));
    let old = current.clone();
    current.value += 1;
    current.elapsed_ms = 1_500;
    assert_eq!(
        counter_delta(Some(&old), &current),
        (Some(2.0), SampleState::Ok)
    );
    current.value = 0;
    assert_eq!(
        counter_delta(Some(&old), &current),
        (None, SampleState::CounterReset)
    );
    current.value = u64::MAX;
    current.epoch = "restarted".into();
    assert_eq!(
        counter_delta(Some(&old), &current),
        (None, SampleState::CounterReset)
    );
    assert_eq!(ratio(Some(0.0), Some(0.0)), None);
    assert_eq!(ratio(Some(9.0), Some(1.0)), Some(0.9));
}

#[test]
fn adaptive_backoff_minima_and_completion_scheduling() {
    assert_eq!(Source::Statements.floor_ms(2_000), 15_000);
    assert_eq!(Source::Replication.floor_ms(2_000), 60_000);
    let mut cadence = Cadence::new(5_000);
    for now in [0, 5_000, 10_000] {
        cadence.complete(now, false, false, 1);
    }
    assert_eq!(cadence.interval_ms(), 10_000);
    assert!(!cadence.due(19_999));
    assert!(cadence.due(20_000));
    cadence.complete(20_000, true, false, 10);
    assert_eq!(cadence.interval_ms(), 5_000);
    for _ in 0..10 {
        cadence.complete(30_000, false, true, 1_000);
    }
    assert_eq!(cadence.interval_ms(), 60_000);
    assert_eq!(cadence.deadline_ms(), 1_000);
}

#[test]
fn unknown_gap_and_reset_arrow_cells_are_null_never_zero() -> Result<(), Box<dyn std::error::Error>>
{
    let mut store = LiveStore::default();
    store.ingest(Source::Database, reading("10", "a"), 0, 0.0);
    store.ingest(Source::Database, reading("12", "a"), 1_000, 1_000.0);
    store.ingest(
        Source::Database,
        Reading::missing(SampleState::Gap),
        2_000,
        2_000.0,
    );
    store.ingest(Source::Database, reading("15", "a"), 3_000, 3_000.0);
    store.ingest(Source::Database, reading("1", "b"), 4_000, 4_000.0);
    let batch = batch(store.window(None, 1_000, false)?)?;
    let values = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or("value type")?;
    let states = batch
        .column(3)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("state type")?;
    for index in 0..values.len() {
        if states.value(index) != "ok" {
            assert!(values.is_null(index));
        }
    }
    assert!(states.iter().flatten().any(|state| state == "gap"));
    assert!(states.iter().flatten().any(|state| state == "counterReset"));
    let units = batch
        .column(4)
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or("units")?;
    assert!(
        units
            .iter()
            .flatten()
            .all(|unit| unit == "count/s" || unit == "fraction")
    );
    Ok(())
}

#[test]
fn series_timestamp_window_identity_and_memory_caps() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    for index in 0..1_000 {
        store.ingest(
            Source::Database,
            reading(&index.to_string(), "a"),
            index * 2_000,
            0.0,
        );
    }
    let batch = batch(store.window(None, 1_000, false)?)?;
    assert!(batch.num_rows() <= 600);
    assert!(store.retained_bytes() < 4 * 1_024 * 1_024);
    let mut sample = reading("1", "a");
    sample.rows = (0..1_500)
        .map(|index| vec![Some(index.to_string()), Some("1".into())])
        .collect();
    store.ingest(Source::Database, sample, 3_000_000, 0.0);
    assert!(batch_from(&store, Source::Database)?.num_rows() <= 1_000);
    assert!(store.retained_bytes() < 4 * 1_024 * 1_024);
    Ok(())
}
fn batch_from(
    store: &LiveStore,
    source: Source,
) -> Result<arrow_array::RecordBatch, Box<dyn std::error::Error>> {
    batch(store.window(Some(source), 1_000, false)?)
}

#[test]
fn query_normalisation_golden_and_utility_fail_closed_before_arrow()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "SELECT 'test-only', E'test-only', 1.2e-3, $$test-only$$, $1 /* test-only */",
            Some("SELECT ?, ?, ?, ?, ?"),
        ),
        (
            "SELECT DATE 'test-only', B'101', X'ff', true, NULL",
            Some("SELECT DATE ?, B?, X?, ?, ?"),
        ),
        ("COPY t TO 'test-only'", None),
        ("ALTER ROLE test_role PASSWORD 'test-only'", None),
        ("SELECT \"test-only\"", None),
        ("SELECT 'unfinished", None),
        ("SELECT $tag$unfinished", None),
        ("SELECT 1 /* unfinished", None),
        ("SELECT 1; ALTER ROLE test_role PASSWORD test_only", None),
    ];
    for (sql, expected) in cases {
        assert_eq!(
            openrows_sql::normalize_monitor_query(sql).as_deref(),
            expected
        );
        let mut store = LiveStore::default();
        let mut sample = reading("1", "a");
        sample.columns[1] = column("query", MetricKind::Query);
        sample.rows[0][1] = Some(sql.into());
        store.ingest(Source::Database, sample, 0, 0.0);
        let batch = batch_from(&store, Source::Database)?;
        let text = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("query type")?;
        assert_eq!(text.iter().next().flatten(), expected);
    }
    Ok(())
}

#[test]
fn scalar_series_caps_pages_and_oldest_expiry() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    let mut sample = reading("1", "a");
    sample
        .columns
        .extend((0..100).map(|index| column(&format!("metric_{index}"), MetricKind::Gauge)));
    sample.rows[0].extend((0..100).map(|_| Some("1".into())));
    store.ingest(Source::Database, sample, 0, 0.0);
    assert_eq!(batch(store.window(None, 1_000, false)?)?.num_rows(), 64);
    let first = batch(store.windowed(openrows_monitor::WindowOptions {
        source: None,
        row_start: 0,
        row_count: 10,
        mean: false,
    })?)?;
    let second = batch(store.windowed(openrows_monitor::WindowOptions {
        source: None,
        row_start: 10,
        row_count: 10,
        mean: false,
    })?)?;
    assert_eq!(first.num_rows(), 10);
    assert_eq!(second.num_rows(), 10);
    assert_ne!(first.column(0), second.column(0));
    store.ingest(Source::Database, reading("2", "a"), 600_001, 600_001.0);
    assert!(batch(store.window(None, 1_000, false)?)?.num_rows() < 4);
    Ok(())
}

#[test]
fn resource_rates_keep_cumulative_values_and_explicit_baselines()
-> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    store.ingest(Source::Io, reading("10", "a"), 0, 0.0);
    let first = batch_from(&store, Source::Io)?;
    assert!(first.column(2).is_null(0));
    store.ingest(Source::Io, reading("20", "a"), 1_000, 1_000.0);
    let second = batch_from(&store, Source::Io)?;
    let values = |index| second.column(index).as_any().downcast_ref::<Float64Array>();
    assert!((values(1).ok_or("counter")?.value(0) - 20.0).abs() < f64::EPSILON);
    assert!((values(2).ok_or("rate")?.value(0) - 10.0).abs() < f64::EPSILON);
    assert_eq!(
        second
            .schema()
            .field(2)
            .metadata()
            .get("unit")
            .map(String::as_str),
        Some("count/s")
    );
    store.ingest(Source::Io, reading("1", "b"), 2_000, 2_000.0);
    assert!(batch_from(&store, Source::Io)?.column(2).is_null(0));
    Ok(())
}

#[test]
fn unavailable_source_has_empty_arrow_with_explicit_state() -> Result<(), Box<dyn std::error::Error>>
{
    let mut store = LiveStore::default();
    store.ingest(
        Source::Statements,
        Reading::missing(SampleState::PermissionDenied("test-only".into())),
        0,
        0.0,
    );
    let batch = batch_from(&store, Source::Statements)?;
    assert_eq!(batch.num_rows(), 0);
    assert_eq!(
        batch
            .schema()
            .metadata()
            .get("sampleState")
            .map(String::as_str),
        Some("permissionDenied")
    );
    Ok(())
}
