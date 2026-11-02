// SPDX-License-Identifier: Apache-2.0
//! Reviewed scalar names and value rules for monitor history; nothing else persists.
use super::{StoreError, monitor_history::MonitorAggregate};

const DAY_MS: i64 = 86_400_000;

/// `ClickHouse` node event rates and gauges.
const CLICKHOUSE: [&str; 21] = [
    "Database:select_queries",
    "Database:insert_queries",
    "Database:failed_queries",
    "Database:selected_rows",
    "Database:selected_bytes",
    "Database:inserted_rows",
    "Database:inserted_bytes",
    "Database:merges",
    "Database:merged_rows",
    "Database:delayed_inserts",
    "Database:rejected_inserts",
    "Database:running_queries",
    "Database:running_merges",
    "Database:tcp_connections",
    "Database:http_connections",
    "Database:memory_tracked_bytes",
    "Database:readonly_replicas",
    "Database:max_parts_per_partition",
    "Database:total_parts",
    "Database:total_bytes",
    "Database:replicas_max_delay_ms",
];

/// Reviewed scalar aggregates; source tables and statement text never persist.
const METRICS: &[&str] = &[
    "sessions:active",
    "sessions:idle",
    "sessions:idle in transaction",
    "sessions:idle in transaction (aborted)",
    "sessions:unknown",
    "sessions:disabled",
    "sessions:fastpath function call",
    "sessions:waiting",
    "Database:backends",
    "Database:xact_commit",
    "Database:xact_rollback",
    "Database:queries",
    "Database:threads_connected",
    "Database:threads_running",
    "Database:buffer_pool_hit_ratio",
    "Database:buffer_read_requests",
    "Database:buffer_reads",
    "Database:rows_read",
    "Database:rows_inserted",
    "Database:rows_updated",
    "Database:rows_deleted",
    "Database:temp_disk_tables",
    "Database:slow_queries",
    "Database:aborted_connects",
    "Database:connections",
    "Database:uptime",
    "Database:since_flush",
    "Database:batch_requests",
    "Database:compilations",
    "Database:recompilations",
    "Database:user_connections",
    "Database:processes_blocked",
    "Database:lock_waits",
    "Database:page_life_expectancy",
    "Database:total_server_memory_kb",
    "Database:target_server_memory_kb",
    "Database:memory_grants_pending",
    "Database:buffer_cache_hit_ratio",
    "Database:cpu_usage",
    "Database:page_size",
    "Database:page_count",
    "Database:freelist_count",
    "Database:file_bytes",
    "Database:wal_bytes",
    "Database:wal_frames",
    "Database:checkpoint_backlog",
    "Database:requests",
    "Database:request_errors",
    "Database:request_ms",
    "Database:rows_returned",
    "Database:round_trip_ms",
    "Database:database_bytes",
    "Database:block_size",
    "Database:total_blocks",
    "Database:used_blocks",
    "Database:free_blocks",
    "Database:memory_bytes",
    "Database:memory_limit_bytes",
    "Database:temp_files",
    "Database:threads",
    "Database:blks_hit",
    "Database:blks_read",
    "Database:temp_files",
    "Database:temp_bytes",
    "Database:deadlocks",
    "Database:tup_returned",
    "Database:tup_fetched",
    "Database:tup_inserted",
    "Database:tup_updated",
    "Database:tup_deleted",
    "Database:cache_hit_ratio",
    "Replication:write_lag_ms",
    "Replication:flush_lag_ms",
    "Replication:replay_lag_ms",
];

/// Whether `metric` is a reviewed scalar name; unknown names never reach the store.
pub(super) fn known(metric: &str) -> bool {
    METRICS.contains(&metric) || CLICKHOUSE.contains(&metric)
}

/// Refuse invalid states, non-finite values, values on missing samples and times
/// outside the retained day.
pub(super) fn validate(point: &MonitorAggregate, now: i64) -> Result<(), StoreError> {
    let state = matches!(
        point.state.as_str(),
        "ok" | "unknown" | "gap" | "counterReset" | "permissionDenied" | "unavailable"
    );
    let value =
        point.value.is_none_or(f64::is_finite) && (point.state == "ok" || point.value.is_none());
    if !state
        || !value
        || point.collected_ms > now
        || point.collected_ms < now.saturating_sub(DAY_MS)
    {
        return Err(StoreError::Corrupt {
            column: "monitor aggregate",
        });
    }
    Ok(())
}
