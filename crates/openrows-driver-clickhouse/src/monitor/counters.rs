// SPDX-License-Identifier: Apache-2.0
//! Whitelisted node counters/gauges; uptime decreases reset baselines, absent names stay unknown.
use super::{columns, guard::Bounded};
use openrows_monitor::{Reading, SampleState};
use std::collections::BTreeMap;

/// Cumulative `system.events` counters since server start.
const EVENTS: [(&str, &str); 12] = [
    ("Query", "queries"),
    ("SelectQuery", "select_queries"),
    ("InsertQuery", "insert_queries"),
    ("FailedQuery", "failed_queries"),
    ("SelectedRows", "selected_rows"),
    ("SelectedBytes", "selected_bytes"),
    ("InsertedRows", "inserted_rows"),
    ("InsertedBytes", "inserted_bytes"),
    ("Merge", "merges"),
    ("MergedRows", "merged_rows"),
    ("DelayedInserts", "delayed_inserts"),
    ("RejectedInserts", "rejected_inserts"),
];
/// Instantaneous `system.metrics` gauges.
const METRICS: [(&str, &str); 6] = [
    ("Query", "running_queries"),
    ("Merge", "running_merges"),
    ("TCPConnection", "tcp_connections"),
    ("HTTPConnection", "http_connections"),
    ("MemoryTracking", "memory_tracked_bytes"),
    ("ReadonlyReplica", "readonly_replicas"),
];
/// Server-refreshed `system.asynchronous_metrics` gauges.
const ASYNCHRONOUS: [(&str, &str); 4] = [
    ("MaxPartCountForPartition", "max_parts_per_partition"),
    ("TotalPartsOfMergeTreeTables", "total_parts"),
    ("TotalBytesOfMergeTreeTables", "total_bytes"),
    ("ReplicasMaxAbsoluteDelay", "replicas_max_delay_ms"),
];
pub(super) const TABLES: [(&str, &str, &str); 3] = [
    ("event", "system.events", "event"),
    ("metric", "system.metrics", "metric"),
    ("async", "system.asynchronous_metrics", "metric"),
];

/// One union over the readable tables; denied tables are left out by discovery.
pub(super) fn sql(readable: [bool; 3]) -> String {
    let mut parts = vec!["SELECT 'uptime', toString(uptime())".to_owned()];
    for ((prefix, table, key), names) in TABLES
        .iter()
        .zip([&EVENTS[..], &METRICS[..], &ASYNCHRONOUS[..]])
        .zip(readable)
        .filter_map(|(pair, readable)| readable.then_some(pair))
    {
        let list: Vec<_> = names.iter().map(|(name, _)| format!("'{name}'")).collect();
        parts.push(format!(
            "SELECT concat('{prefix}:', {key}), toString(value) FROM {table} WHERE {key} IN ({})",
            list.join(", ")
        ));
    }
    parts.join(" UNION ALL ")
}
/// Probe statement for one counter table.
pub(super) fn probe(index: usize) -> Option<String> {
    let (_, table, key) = TABLES.get(index)?;
    Some(format!("SELECT {key}, value FROM {table} LIMIT 0"))
}

#[derive(Default)]
pub(super) struct Epoch {
    previous: Option<f64>,
    generation: u64,
}
impl Epoch {
    pub(super) fn observe(&mut self, uptime: Option<f64>) -> String {
        let Some(uptime) = uptime.filter(|v| v.is_finite()) else {
            return "best effort".into();
        };
        if self.previous.is_some_and(|old| uptime < old) {
            self.generation = self.generation.saturating_add(1);
        }
        self.previous = Some(uptime);
        self.generation.to_string()
    }
}
pub(super) fn reading(result: Bounded, epoch: &mut Epoch) -> Reading {
    let values: BTreeMap<_, _> = result
        .rows
        .into_iter()
        .filter_map(|r| Some((r.first()?.clone()?, r.get(1)?.clone())))
        .collect();
    let value = |key: &str| values.get(key).cloned().flatten();
    let marker = epoch.observe(value("uptime").and_then(|v| v.parse().ok()));
    let mut row = vec![Some("server".to_owned())];
    let mut spec = String::from("server_id:text");
    let groups = [
        ("event", &EVENTS[..], "counter"),
        ("metric", &METRICS[..], "count"),
        ("async", &ASYNCHRONOUS[..], "count"),
    ];
    for (prefix, names, unit) in groups {
        for (name, column) in names {
            let mut cell = value(&format!("{prefix}:{name}"));
            if *column == "replicas_max_delay_ms" {
                cell = cell
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| (v * 1_000.0).to_string());
            }
            row.push(cell);
            let unit = match unit {
                "count" if column.ends_with("bytes") => "bytes",
                "count" if column.ends_with("ms") => "ms",
                other => other,
            };
            for part in [",", column, ":", unit] {
                spec.push_str(part);
            }
        }
    }
    row.push(value("uptime"));
    spec.push_str(",uptime:s");
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: marker,
        columns: columns(&spec),
        rows: vec![row],
        partial: result.partial,
    }
}
