// SPDX-License-Identifier: Apache-2.0
//! Weighted deltas, regressions, privacy and bounded retention over real interval semantics.
use openrows_monitor::{Column, LiveStore, MetricKind, Reading, SampleState, Source};

fn reading(calls: u64, total: u64, epoch: &str) -> Reading {
    let spec = [
        "statement_id",
        "calls",
        "total_ms",
        "cumulative_mean_ms",
        "max_since_reset_ms",
        "rows",
        "shared_blks_hit",
        "shared_blks_read",
        "shared_blks_dirtied",
        "shared_blks_written",
        "temp_blks_read",
        "temp_blks_written",
        "query",
    ];
    let columns = spec
        .iter()
        .enumerate()
        .map(|(i, name)| Column {
            name: (*name).into(),
            unit: "count".into(),
            kind: match i {
                0 => MetricKind::Text,
                3 | 4 => MetricKind::Gauge,
                12 => MetricKind::Query,
                _ => MetricKind::Counter,
            },
        })
        .collect();
    let row = [
        "q".into(),
        calls.to_string(),
        total.to_string(),
        "5".into(),
        "99".into(),
        calls.to_string(),
        calls.to_string(),
        "0".into(),
        "0".into(),
        "0".into(),
        "0".into(),
        "0".into(),
        "SELECT * FROM events WHERE key = 'test-only'".into(),
    ];
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.into(),
        partial: false,
        columns,
        rows: vec![row.into_iter().map(Some).collect()],
    }
}
#[test]
fn weighted_totals_mean_source_maximum_and_no_invented_percentiles() -> Result<(), serde_json::Error>
{
    let mut store = LiveStore::default();
    for (elapsed, calls, total) in [(0, 0, 0), (15_000, 5, 50), (30_000, 20, 500)] {
        store.ingest(
            Source::Statements,
            reading(calls, total, "epoch"),
            elapsed,
            f64::from(u32::try_from(elapsed).unwrap_or(0)),
        );
    }
    let detail = store.investigate("q");
    assert_eq!(detail.calls, Some(20.0));
    assert_eq!(detail.total_ms, Some(500.0));
    assert_eq!(detail.mean_ms, Some(25.0));
    assert_eq!(detail.max_since_reset_ms, Some(99.0));
    assert_eq!(detail.regressions.len(), 1);
    assert_eq!(
        detail.query.as_deref(),
        Some("SELECT * FROM events WHERE key = ?")
    );
    let json = serde_json::to_string(&detail)?;
    assert!(!json.contains("test-only") && !json.contains("percentile") && !json.contains("p95"));
    Ok(())
}
#[test]
fn regressions_require_both_thresholds_minimum_calls_and_complete_adjacent_intervals() {
    for (calls, total, expected) in [(5, 100, 1), (5, 99, 0), (4, 100, 0), (5, 50, 0)] {
        let mut store = LiveStore::default();
        store.ingest(Source::Statements, reading(0, 0, "a"), 0, 0.0);
        store.ingest(Source::Statements, reading(5, 50, "a"), 15_000, 15_000.0);
        store.ingest(
            Source::Statements,
            reading(5 + calls, 50 + total, "a"),
            30_000,
            30_000.0,
        );
        assert_eq!(store.investigate("q").regressions.len(), expected);
    }
    let mut store = LiveStore::default();
    store.ingest(Source::Statements, reading(0, 0, "a"), 0, 0.0);
    store.ingest(Source::Statements, reading(5, 50, "a"), 15_000, 15_000.0);
    store.ingest(Source::Statements, reading(10, 500, "b"), 30_000, 30_000.0);
    assert!(store.investigate("q").regressions.is_empty());
    assert_eq!(
        store.investigate("q").intervals[2].state,
        SampleState::CounterReset
    );
}
#[test]
fn absence_and_permission_loss_cannot_retain_usable_query_advice() {
    let mut store = LiveStore::default();
    store.ingest(Source::Statements, reading(0, 0, "a"), 0, 0.0);
    store.ingest(Source::Statements, reading(5, 50, "a"), 15_000, 15_000.0);
    store.ingest(
        Source::Statements,
        Reading {
            rows: vec![],
            ..reading(0, 0, "a")
        },
        30_000,
        30_000.0,
    );
    store.ingest(
        Source::Statements,
        reading(100, 1000, "a"),
        45_000,
        45_000.0,
    );
    assert_eq!(
        store.investigate("q").intervals.last().map(|i| &i.state),
        Some(&SampleState::Unknown)
    );
    store.ingest(
        Source::Statements,
        Reading::missing(SampleState::PermissionDenied("restricted".into())),
        60_000,
        60_000.0,
    );
    assert!(store.investigate("q").intervals.is_empty());
    assert!(store.investigate("q").query.is_none());
}
#[test]
fn per_statement_history_and_plans_remain_in_the_shared_byte_cap() {
    let mut store = LiveStore::default();
    for tick in 0..100_u32 {
        let mut reading = reading(u64::from(tick), u64::from(tick) * 10, "a");
        reading.rows = (0..200)
            .map(|index| {
                let mut row = reading.rows[0].clone();
                row[0] = Some(format!("q:{index}"));
                row
            })
            .collect();
        store.ingest(
            Source::Statements,
            reading,
            u64::from(tick) * 15_000,
            f64::from(tick) * 15_000.0,
        );
        assert!(store.retained_bytes() <= 2 * 1024 * 1024);
    }
    assert!(store.investigate("q:199").intervals.len() <= 41);
}

#[test]
fn missing_metrics_and_partial_snapshots_remain_unknown() {
    let mut store = LiveStore::default();
    store.ingest(Source::Statements, reading(0, 0, "a"), 0, 0.0);
    let mut sample = reading(5, 100, "a");
    sample.rows[0][2] = None;
    store.ingest(Source::Statements, sample, 15000, 15000.0);
    assert!(store.investigate("q").total_ms.is_none());
    assert!(store.investigate("q").mean_ms.is_none());
    let mut sample = reading(10, 1000, "a");
    sample.partial = true;
    store.ingest(Source::Statements, sample, 30000, 30000.0);
    assert!(store.investigate("q").regressions.is_empty());
    assert!(store.investigate("q").partial);
}
#[test]
fn regressions_never_cross_missing_observations() {
    let mut store = LiveStore::default();
    store.ingest(Source::Statements, reading(0, 0, "a"), 0, 0.0);
    store.ingest(Source::Statements, reading(5, 50, "a"), 15000, 15000.0);
    store.ingest(Source::Statements, reading(10, 500, "a"), 30000, 45000.0);
    assert!(store.investigate("q").regressions.is_empty());
}
#[test]
fn repeated_blocking_observations_end_on_permission_loss_and_changed_statement() {
    let spec = [
        "pid",
        "state",
        "wait",
        "age_ms",
        "transaction_ms",
        "blockers",
        "query",
        "user",
        "database",
        "application",
        "query_start",
    ];
    let sample = Reading {
        columns: spec
            .iter()
            .map(|name| Column {
                name: (*name).into(),
                unit: "text".into(),
                kind: MetricKind::Text,
            })
            .collect(),
        rows: vec![
            [
                "7",
                "active",
                "Lock: transactionid",
                "2000",
                "3000",
                "{8}",
                "UPDATE t SET a = ?",
                "role",
                "db",
                "fixture",
                "start",
            ]
            .map(|value| Some(value.into()))
            .to_vec(),
        ],
        state: SampleState::Ok,
        epoch: "a".into(),
        partial: false,
        terminal: false,
    };
    let mut store = LiveStore::default();
    for tick in 0..3_u32 {
        store.ingest(
            Source::Activity,
            sample.clone(),
            u64::from(tick) * 5000,
            f64::from(tick) * 5000.0,
        );
    }
    assert!(
        openrows_monitor::advisor::advise(&store.advisor_input(10000.0))
            .iter()
            .any(|card| card.rule_id == "pg.lock-hotspot")
    );
    let mut changed = sample;
    changed.rows[0][10] = Some("changed".into());
    store.ingest(Source::Activity, changed, 15000, 15000.0);
    assert!(openrows_monitor::advisor::advise(&store.advisor_input(15000.0)).is_empty());
    store.ingest(
        Source::Activity,
        Reading::missing(SampleState::PermissionDenied("restricted".into())),
        20000,
        20000.0,
    );
    assert!(store.advisor_input(20000.0).locks.is_empty());
}

#[test]
fn top_rankings_use_retained_window_not_only_the_latest_interval()
-> Result<(), Box<dyn std::error::Error>> {
    let mut store = LiveStore::default();
    for (tick, first, second) in [(0_u32, 0, 0), (1, 1000, 10), (2, 1001, 510)] {
        let mut sample = reading(u64::from(tick) * 10, first, "a");
        let mut other = reading(u64::from(tick) * 10, second, "a").rows.remove(0);
        other[0] = Some("other".into());
        sample.rows.push(other);
        store.ingest(
            Source::Statements,
            sample,
            u64::from(tick) * 15000,
            f64::from(tick) * 15000.0,
        );
    }
    let bytes = store.window(Some(Source::Statements), 20, false)?;
    let batch = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(bytes), None)?
        .next()
        .ok_or("batch")??;
    let ids = batch
        .column_by_name("statement_id")
        .ok_or("identity")?
        .as_any()
        .downcast_ref::<arrow_array::StringArray>()
        .ok_or("identity type")?;
    assert_eq!(ids.value(0), "q");
    let totals = batch
        .column_by_name("live_window_total_ms")
        .ok_or("window total")?
        .as_any()
        .downcast_ref::<arrow_array::Float64Array>()
        .ok_or("total type")?;
    // Rates are projected back to deltas; allow four relative floating-point ULPs.
    assert!((totals.value(0) - 1001.0).abs() <= 1001.0 * f64::EPSILON * 4.0);
    assert!((totals.value(1) - 510.0).abs() <= 510.0 * f64::EPSILON * 4.0);
    Ok(())
}
