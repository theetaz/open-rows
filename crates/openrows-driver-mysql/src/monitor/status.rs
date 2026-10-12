// SPDX-License-Identifier: Apache-2.0
//! Whitelisted global counters and uptime/FLUSH STATUS continuity, never fabricated zeros.
use super::{bounded::Bounded, layout::columns};
use openrows_monitor::{Reading, SampleState};
use std::collections::BTreeMap;

const COUNTERS: [(&str, &str); 16] = [
    ("Queries", "queries"),
    ("Threads_connected", "threads_connected"),
    ("Threads_running", "threads_running"),
    ("Innodb_buffer_pool_read_requests", "buffer_read_requests"),
    ("Innodb_buffer_pool_reads", "buffer_reads"),
    ("Innodb_rows_read", "rows_read"),
    ("Innodb_rows_inserted", "rows_inserted"),
    ("Innodb_rows_updated", "rows_updated"),
    ("Innodb_rows_deleted", "rows_deleted"),
    ("Created_tmp_disk_tables", "temp_disk_tables"),
    ("Slow_queries", "slow_queries"),
    ("Aborted_connects", "aborted_connects"),
    ("Innodb_deadlocks", "deadlocks"),
    ("Uptime", "uptime"),
    ("Uptime_since_flush_status", "since_flush"),
    ("Connections", "connections"),
];
/// `MySQL` has no deadlock status variable; its `InnoDB` metric needs PROCESS.
pub(super) fn mysql() -> String {
    let names: Vec<_> = COUNTERS
        .iter()
        .map(|(name, _)| format!("'{name}'"))
        .collect();
    format!(
        "SELECT VARIABLE_NAME, VARIABLE_VALUE FROM performance_schema.global_status WHERE VARIABLE_NAME IN ({}) UNION ALL SELECT 'Innodb_deadlocks', COUNT FROM information_schema.INNODB_METRICS WHERE NAME = 'lock_deadlocks' AND STATUS = 'enabled'",
        names.join(", ")
    )
}
/// Whether the probe returned the enabled deadlock metric.
pub(super) fn has_deadlocks(result: &Bounded) -> bool {
    result
        .rows
        .iter()
        .any(|row| row.first().and_then(Option::as_deref) == Some("Innodb_deadlocks"))
}
#[derive(Default)]
pub(super) struct Epoch {
    previous: Option<(u64, Option<u64>)>,
    generation: u64,
}
impl Epoch {
    pub(super) fn observe(&mut self, uptime: Option<u64>, flush: Option<u64>) -> String {
        let Some(uptime) = uptime else {
            return "best effort".into();
        };
        if self.previous.is_some_and(|(old, old_flush)| {
            uptime < old || flush.zip(old_flush).is_some_and(|(new, old)| new < old)
        }) {
            self.generation = self.generation.saturating_add(1);
        }
        self.previous = Some((uptime, flush));
        self.generation.to_string()
    }
}
pub(super) fn reading(result: Bounded, epoch: &mut Epoch) -> Reading {
    let values: BTreeMap<_, _> = result
        .rows
        .into_iter()
        .filter_map(|r| {
            Some((
                r.first()?.as_deref()?.to_ascii_lowercase(),
                r.get(1)?.clone(),
            ))
        })
        .collect();
    let value = |name: &str| values.get(&name.to_ascii_lowercase()).cloned().flatten();
    let marker = epoch.observe(
        value("Uptime").and_then(|v| v.parse().ok()),
        value("Uptime_since_flush_status").and_then(|v| v.parse().ok()),
    );
    let mut row = vec![Some("server".into())];
    row.extend(COUNTERS.iter().map(|(name, _)| value(name)));
    let mut spec = String::from("server_id:text");
    for (_, name) in COUNTERS {
        let unit = if matches!(
            name,
            "threads_connected" | "threads_running" | "uptime" | "since_flush"
        ) {
            "count"
        } else {
            "counter"
        };
        spec.push(',');
        spec.push_str(name);
        spec.push(':');
        spec.push_str(unit);
    }
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: marker,
        columns: columns(&spec),
        rows: vec![row],
        partial: result.partial,
    }
}
