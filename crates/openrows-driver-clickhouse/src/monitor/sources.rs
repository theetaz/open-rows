// SPDX-License-Identifier: Apache-2.0
//! Fixed system-table projections; `{p1:String}` excludes this collector's own queries.
use super::{Config, guard::Bounded};
use openrows_monitor::{Column, Reading, SampleState, Source};

/// `elapsed` is seconds; every query cell is bounded before core redaction.
const ACTIVITY: &str = "SELECT concat(hostName(), ':', query_id), 'active', NULL, toUInt64(elapsed * 1000), if(length(query) < 4096, query, NULL), user, current_database, if(is_initial_query, 'initial', 'secondary'), hostName(), memory_usage, peak_memory_usage, read_rows, read_bytes, written_rows FROM {from} WHERE NOT startsWith(initial_query_id, {p1:String}) ORDER BY elapsed DESC, query_id LIMIT 1000";
/// Terminal events only (no `QueryStart`), in an hourly anchored window that
/// prunes by date and time; the anchor is the counters' reset epoch.
const STATEMENTS: &str = "WITH toStartOfHour(now()) AS anchor SELECT concat(current_database, ':', toString(normalized_query_hash), ':', if(is_initial_query, 'initial', 'secondary')), count(), sum(query_duration_ms), avg(query_duration_ms), max(query_duration_ms), sum(read_rows), sum(read_bytes), sum(result_rows), max(memory_usage), countIf(type = 'ExceptionWhileProcessing'), nullIf(normalizeQuery(any(if(length(query) < 4096, query, ''))), ''), toString(any(anchor)) FROM system.query_log WHERE event_date >= toDate(anchor) AND event_time >= anchor AND type IN ('QueryFinish', 'ExceptionWhileProcessing') AND current_database = currentDatabase() AND NOT startsWith(initial_query_id, {p1:String}) GROUP BY current_database, normalized_query_hash, is_initial_query ORDER BY sum(query_duration_ms) DESC, normalized_query_hash LIMIT 200";
const TABLES: &str = "SELECT concat(database, '.', table), database, table, sum(parts), count(), max(parts), sum(rows), sum(bytes), sum(compressed), sum(uncompressed) FROM (SELECT database, table, partition_id, count() AS parts, sum(rows) AS rows, sum(bytes_on_disk) AS bytes, sum(data_compressed_bytes) AS compressed, sum(data_uncompressed_bytes) AS uncompressed FROM system.parts WHERE active AND database = currentDatabase() GROUP BY database, table, partition_id) GROUP BY database, table ORDER BY sum(bytes) DESC, table LIMIT 1000";
const MERGES: &str = "SELECT concat(database, '.', table, ':', result_part_name), database, table, if(is_mutation, 'mutation', merge_type), toUInt64(elapsed * 1000), progress, num_parts, rows_read, rows_written, bytes_read_uncompressed, memory_usage FROM system.merges ORDER BY elapsed DESC, result_part_name LIMIT 1000";
const REPLICAS: &str = "SELECT concat(database, '.', table), multiIf(is_readonly, 'readonly', is_session_expired, 'session expired', 'ok'), queue_size, inserts_in_queue, merges_in_queue, absolute_delay * 1000 FROM system.replicas ORDER BY absolute_delay DESC, database, table LIMIT 1000";

pub(super) fn query(source: Source, config: &Config) -> (String, &'static str) {
    match source {
        Source::Activity => (
            ACTIVITY.replace("{from}", &from(config.cluster.as_deref())),
            "pid:text,state:text,wait:text,age_ms:ms,query:query,user:text,database:text,query_kind:text,node:text,memory_bytes:bytes,peak_memory_bytes:bytes,read_rows:count,read_bytes:bytes,written_rows:count",
        ),
        Source::Database => (super::counters::sql(config.counters), ""),
        Source::Statements => (
            STATEMENTS.into(),
            "statement_id:text,calls:counter,total_ms:counter,cumulative_mean_ms:ms,max_since_reset_ms:ms,rows:counter,read_bytes:counter,result_rows:counter,max_memory_bytes:bytes,errors:counter,query:query",
        ),
        Source::Tables => (
            TABLES.into(),
            "relation_id:text,schema:text,table:text,active_parts:count,partitions:count,max_parts_per_partition:count,rows:count,bytes_on_disk:bytes,compressed_bytes:bytes,uncompressed_bytes:bytes",
        ),
        Source::Io => (
            MERGES.into(),
            "merge_id:text,schema:text,table:text,merge_type:text,elapsed_ms:ms,progress:fraction,source_parts:count,rows_read:count,rows_written:count,bytes_read:bytes,memory_bytes:bytes",
        ),
        Source::Replication => (
            REPLICAS.into(),
            "channel_id:text,state:text,queue_size:count,inserts_in_queue:count,merges_in_queue:count,replay_lag_ms:ms",
        ),
        Source::Indexes => ("SELECT 1".into(), ""),
    }
}
/// The connected node, or every replica of a probed, granted cluster.
pub(super) fn from(cluster: Option<&str>) -> String {
    cluster.map_or_else(
        || "system.processes".into(),
        |name| format!("clusterAllReplicas('{name}', system.processes)"),
    )
}
/// Accept plain cluster identifiers only; they are embedded as a literal.
pub(super) fn cluster_name(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
    .then_some(name)
}
/// Statement counters carry the hourly window anchor as their trailing epoch.
pub(super) fn reading(source: Source, result: Bounded, columns: Vec<Column>) -> Reading {
    let mut rows = result.rows;
    let mut epoch = String::from("best effort");
    if source == Source::Statements {
        epoch = rows
            .first()
            .and_then(|row| row.last().cloned().flatten())
            .unwrap_or(epoch);
        for row in &mut rows {
            row.truncate(columns.len());
        }
    }
    let limit = if source == Source::Statements {
        200
    } else {
        1_000
    };
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch,
        partial: result.partial || rows.len() >= limit,
        rows,
        columns,
    }
}
/// No replicated tables is "not applicable", never a zero lag.
pub(super) fn replication(result: Bounded, columns: Vec<Column>) -> Reading {
    let state = if result.rows.is_empty() {
        SampleState::Unavailable("Not applicable: no replicated tables on this node".into())
    } else {
        SampleState::Ok
    };
    Reading {
        terminal: false,
        state,
        epoch: "best effort".into(),
        partial: result.partial || result.rows.len() >= 1_000,
        rows: result.rows,
        columns,
    }
}
