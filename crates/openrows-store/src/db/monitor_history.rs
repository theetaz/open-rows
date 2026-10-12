// SPDX-License-Identifier: Apache-2.0
//! Fixed, default-off scalar history; SQL text and source rows cannot enter this store.
use super::{LocalStore, StoreError, error::sqlite};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const DAY_MS: i64 = 86_400_000;
// A conservative 2-KiB charge covers bounded keys, both indexes and page overhead.
const CONNECTION_ROWS: i64 = 50 * 1_024 * 1_024 / 2_048;
const APP_ROWS: i64 = 100 * 1_024 * 1_024 / 2_048;

/// One aggregate observation, without statement text, identities or result rows.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorAggregate {
    /// Allowlisted scalar metric name (database identities are removed).
    pub metric: String,
    /// Wall-clock sample time in milliseconds.
    pub collected_ms: i64,
    /// A real finite value only when the state is ok.
    pub value: Option<f64>,
    /// Small validity label, without server error details.
    pub state: String,
}

impl LocalStore {
    /// Replace the explicit opt-in list and purge disabled profiles atomically.
    /// # Errors
    /// Storage failure; invalid keys are refused.
    pub fn set_monitor_profiles(&self, profiles: &[String], now: i64) -> Result<(), StoreError> {
        if profiles.len() > 1_000 || profiles.iter().any(|id| id.len() > 64) {
            return Err(StoreError::Corrupt {
                column: "monitor profile",
            });
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(sqlite("configure monitor history"))?;
        tx.execute("DELETE FROM monitor_profiles", [])
            .map_err(sqlite("disable history"))?;
        for profile in profiles {
            tx.execute(
                "INSERT OR IGNORE INTO monitor_profiles VALUES (?1)",
                [profile],
            )
            .map_err(sqlite("enable history"))?;
        }
        tx.execute("DELETE FROM monitor_history WHERE profile_id NOT IN (SELECT profile_id FROM monitor_profiles)", [])
            .map_err(sqlite("purge disabled history"))?;
        enforce(&tx, now, CONNECTION_ROWS, APP_ROWS)?;
        tx.commit().map_err(sqlite("configure monitor history"))
    }

    /// Record scalar samples only while this profile is opted in; expire and cap every write.
    /// # Errors
    /// Invalid aggregate or storage failure; the whole batch rolls back.
    pub fn record_monitor(
        &self,
        scope: (&str, &str),
        points: &[MonitorAggregate],
        now: i64,
    ) -> Result<(), StoreError> {
        if scope.0.len() > 64 || scope.1.len() > 256 || points.len() > 64 {
            return Err(StoreError::Corrupt {
                column: "monitor scope",
            });
        }
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(sqlite("record monitor history"))?;
        for point in points {
            validate(point, now)?;
            tx.execute("INSERT OR IGNORE INTO monitor_history SELECT ?1, ?2, ?3, ?4, ?5, ?6 WHERE EXISTS (SELECT 1 FROM monitor_profiles WHERE profile_id = ?1)",
                params![scope.0, scope.1, point.metric, point.collected_ms, point.value, point.state])
                .map_err(sqlite("record monitor aggregate"))?;
        }
        enforce(&tx, now, CONNECTION_ROWS, APP_ROWS)?;
        tx.commit().map_err(sqlite("record monitor history"))
    }

    /// Read at most 300 buckets per metric, preserving missing states within each bucket.
    /// # Errors
    /// Storage failure; ranges are clamped to the retained 24 hours.
    pub fn monitor_history(
        &self,
        scope: (&str, &str),
        range: (i64, i64),
    ) -> Result<Vec<MonitorAggregate>, StoreError> {
        let conn = self.lock()?;
        enforce(&conn, range.1, CONNECTION_ROWS, APP_ROWS)?;
        let since = range.0.max(range.1.saturating_sub(DAY_MS));
        let bucket = (range.1.saturating_sub(since) / 300).saturating_add(1);
        let mut statement = conn.prepare("SELECT metric, ?3 + ((collected_ms - ?3) / ?5) * ?5, value, state FROM (SELECT *, row_number() OVER (PARTITION BY metric, (collected_ms - ?3) / ?5 ORDER BY state != 'ok' DESC, collected_ms DESC) AS position FROM monitor_history WHERE profile_id = ?1 AND database_name = ?2 AND collected_ms >= ?3 AND collected_ms <= ?4) WHERE position = 1 ORDER BY metric, collected_ms LIMIT 19200")
            .map_err(sqlite("read monitor history"))?;
        statement
            .query_map(params![scope.0, scope.1, since, range.1, bucket], |row| {
                Ok(MonitorAggregate {
                    metric: row.get(0)?,
                    collected_ms: row.get(1)?,
                    value: row.get(2)?,
                    state: row.get(3)?,
                })
            })
            .map_err(sqlite("read monitor history"))?
            .collect::<Result<_, _>>()
            .map_err(sqlite("decode monitor history"))
    }

    /// Purge one connection or all connections without changing their opt-in preference.
    /// # Errors
    /// Storage failure.
    pub fn purge_monitor_history(&self, profile: Option<&str>) -> Result<(), StoreError> {
        self.lock()?
            .execute(
                "DELETE FROM monitor_history WHERE ?1 IS NULL OR profile_id = ?1",
                [profile],
            )
            .map(drop)
            .map_err(sqlite("purge monitor history"))
    }
}

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

fn validate(point: &MonitorAggregate, now: i64) -> Result<(), StoreError> {
    let metric = matches!(
        point.metric.as_str(),
        "sessions:active"
            | "sessions:idle"
            | "sessions:idle in transaction"
            | "sessions:idle in transaction (aborted)"
            | "sessions:unknown"
            | "sessions:disabled"
            | "sessions:fastpath function call"
            | "sessions:waiting"
            | "Database:backends"
            | "Database:xact_commit"
            | "Database:xact_rollback"
            | "Database:queries"
            | "Database:threads_connected"
            | "Database:threads_running"
            | "Database:buffer_pool_hit_ratio"
            | "Database:buffer_read_requests"
            | "Database:buffer_reads"
            | "Database:rows_read"
            | "Database:rows_inserted"
            | "Database:rows_updated"
            | "Database:rows_deleted"
            | "Database:temp_disk_tables"
            | "Database:slow_queries"
            | "Database:aborted_connects"
            | "Database:connections"
            | "Database:uptime"
            | "Database:since_flush"
            | "Database:batch_requests"
            | "Database:compilations"
            | "Database:recompilations"
            | "Database:user_connections"
            | "Database:processes_blocked"
            | "Database:lock_waits"
            | "Database:page_life_expectancy"
            | "Database:total_server_memory_kb"
            | "Database:target_server_memory_kb"
            | "Database:memory_grants_pending"
            | "Database:buffer_cache_hit_ratio"
            | "Database:cpu_usage"
            | "Database:blks_hit"
            | "Database:blks_read"
            | "Database:temp_files"
            | "Database:temp_bytes"
            | "Database:deadlocks"
            | "Database:tup_returned"
            | "Database:tup_fetched"
            | "Database:tup_inserted"
            | "Database:tup_updated"
            | "Database:tup_deleted"
            | "Database:cache_hit_ratio"
            | "Replication:write_lag_ms"
            | "Replication:flush_lag_ms"
            | "Replication:replay_lag_ms"
    ) || CLICKHOUSE.contains(&point.metric.as_str());
    let state = matches!(
        point.state.as_str(),
        "ok" | "unknown" | "gap" | "counterReset" | "permissionDenied" | "unavailable"
    );
    let value =
        point.value.is_none_or(f64::is_finite) && (point.state == "ok" || point.value.is_none());
    if !metric
        || !state
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

fn enforce(
    conn: &Connection,
    now: i64,
    connection_rows: i64,
    app_rows: i64,
) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM monitor_history WHERE collected_ms < ?1",
        [now.saturating_sub(DAY_MS)],
    )
    .map_err(sqlite("expire monitor history"))?;
    if conn.query_row("SELECT coalesce(max(total), 0) FROM (SELECT count(*) AS total FROM monitor_history GROUP BY profile_id)", [], |row| row.get::<_, i64>(0)).map_err(sqlite("count connection history"))? > connection_rows {
    conn.execute("DELETE FROM monitor_history WHERE (profile_id, database_name, metric, collected_ms) IN (SELECT profile_id, database_name, metric, collected_ms FROM (SELECT *, row_number() OVER (PARTITION BY profile_id ORDER BY collected_ms DESC, database_name, metric) AS position FROM monitor_history) WHERE position > ?1)", [connection_rows])
        .map_err(sqlite("cap connection history"))?;
    }
    if conn
        .query_row("SELECT count(*) FROM monitor_history", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(sqlite("count app history"))?
        <= app_rows
    {
        return Ok(());
    }
    conn.execute("DELETE FROM monitor_history WHERE (profile_id, database_name, metric, collected_ms) IN (SELECT profile_id, database_name, metric, collected_ms FROM monitor_history ORDER BY collected_ms DESC, profile_id, database_name, metric LIMIT -1 OFFSET ?1)", [app_rows])
        .map(drop).map_err(sqlite("cap app history"))
}

pub(super) fn expire_startup(conn: &Connection) -> Result<(), StoreError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|_| StoreError::Corrupt {
            column: "monitor clock",
        })?;
    let now = i64::try_from(duration.as_millis()).map_err(|_| StoreError::Corrupt {
        column: "monitor clock",
    })?;
    conn.execute(
        "DELETE FROM monitor_history WHERE collected_ms < ?1",
        [now.saturating_sub(DAY_MS)],
    )
    .map(drop)
    .map_err(sqlite("expire monitor history on startup"))
}

#[cfg(test)]
#[path = "monitor_history_tests.rs"]
mod tests;
