// SPDX-License-Identifier: Apache-2.0
//! Guards, explicit missing states, scope and reset semantics without a server.
use super::{
    columns, counters, discovery, failure,
    guard::{self, Bounded},
    sources,
};
use openrows_driver_api::DriverError;
use openrows_monitor::{SampleState, Source};
use serde_json::json;

fn server(number: u32) -> DriverError {
    DriverError::Server {
        code: "HY000".into(),
        message: format!("Code: {number}. DB::Exception: test (ClickHouse error {number})"),
    }
}

#[test]
fn guards_are_applied_before_readonly_and_clamped() {
    let settings = guard::settings(250);
    assert!(settings.contains("max_execution_time = 0.25"));
    assert!(settings.contains("result_overflow_mode = 'throw'"));
    assert!(settings.ends_with("readonly = 1"));
    assert!(guard::settings(60_000).contains("max_execution_time = 1,"));
}

#[test]
fn oversized_cells_are_null_and_partial() {
    let rows = vec![vec![
        json!("a".repeat(4_096)),
        json!(7),
        json!(null),
        json!("x"),
    ]];
    let bounded = guard::bound(rows);
    assert!(bounded.partial);
    assert_eq!(
        bounded.rows,
        vec![vec![None, Some("7".into()), None, Some("x".into())]]
    );
}

#[test]
fn denied_missing_and_bounded_failures_stay_explicit() {
    assert!(matches!(
        failure(&server(497), Source::Statements).state,
        SampleState::PermissionDenied(reason) if reason.contains("system.query_log")
    ));
    assert!(matches!(
        failure(&server(60), Source::Statements).state,
        SampleState::Unavailable(reason) if reason.contains("query_log is not configured")
    ));
    let capped = failure(&server(158), Source::Tables);
    assert_eq!(capped.state, SampleState::Gap);
    assert!(capped.partial);
    assert!(matches!(
        failure(&server(164), Source::Database).state,
        SampleState::Unavailable(reason) if reason.contains("readonly profile")
    ));
    assert!(matches!(
        failure(&DriverError::Auth("denied".into()), Source::Activity).state,
        SampleState::PermissionDenied(_)
    ));
}

#[test]
fn node_scope_is_default_and_cluster_names_are_plain() {
    assert_eq!(sources::from(None), "system.processes");
    assert_eq!(
        sources::from(Some("prod_1")),
        "clusterAllReplicas('prod_1', system.processes)"
    );
    assert_eq!(sources::cluster_name(" prod-1.eu "), Some("prod-1.eu"));
    assert_eq!(sources::cluster_name("x', system.one) --"), None);
    assert_eq!(sources::cluster_name(""), None);
}

#[test]
fn support_floor_is_twenty_five_three() {
    assert!(discovery::supported("25.3.9.72"));
    assert!(discovery::supported("26.1.1.1"));
    assert!(!discovery::supported("24.8.14.39"));
    assert!(!discovery::supported("unknown"));
}

#[test]
fn counters_skip_unreadable_tables_and_never_fabricate_zero() {
    let sql = counters::sql([true, false, true]);
    assert!(sql.contains("system.events"));
    assert!(!sql.contains("system.metrics "));
    assert!(sql.contains("system.asynchronous_metrics"));
    let mut epoch = counters::Epoch::default();
    let result = Bounded {
        rows: vec![
            vec![Some("uptime".into()), Some("100".into())],
            vec![Some("event:Query".into()), Some("0".into())],
            vec![
                Some("async:ReplicasMaxAbsoluteDelay".into()),
                Some("2".into()),
            ],
        ],
        partial: false,
    };
    let reading = counters::reading(result, &mut epoch);
    let cell = |name: &str| {
        let index = reading.columns.iter().position(|c| c.name == name)?;
        reading.rows.first()?.get(index).cloned().flatten()
    };
    assert_eq!(cell("queries").as_deref(), Some("0"));
    assert_eq!(cell("running_queries"), None);
    assert_eq!(cell("replicas_max_delay_ms").as_deref(), Some("2000"));
    assert_eq!(reading.epoch, "0");
    assert_eq!(epoch.observe(Some(5.0)), "1");
    assert_eq!(epoch.observe(None), "best effort");
}

#[test]
fn statement_window_anchor_is_the_epoch() {
    let spec = "statement_id:text,calls:counter,query:query";
    let result = Bounded {
        rows: vec![vec![
            Some("db:1:initial".into()),
            Some("3".into()),
            Some("SELECT ?".into()),
            Some("2026-10-12 16:00:00".into()),
        ]],
        partial: false,
    };
    let reading = sources::reading(Source::Statements, result, columns(spec));
    assert_eq!(reading.epoch, "2026-10-12 16:00:00");
    assert_eq!(reading.rows[0].len(), 3);
}

#[test]
fn no_replicated_tables_is_not_applicable() {
    let reading = sources::replication(Bounded::default(), columns("channel_id:text"));
    assert!(matches!(reading.state, SampleState::Unavailable(r) if r.contains("Not applicable")));
}
