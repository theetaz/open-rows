// SPDX-License-Identifier: Apache-2.0
//! Explicit, bounded, fixed `PostgreSQL` statistics projections.
use super::Config;
use openrows_monitor::{Column, MetricKind, Source};

const EPOCH: &str = "pg_postmaster_start_time()::text";
const DB_EPOCH: &str = "pg_postmaster_start_time()::text || COALESCE((SELECT stats_reset::text FROM pg_stat_database WHERE datname = current_database()), '')";

pub(super) fn query(source: Source, config: &Config) -> Option<(String, Vec<Column>)> {
    let (sql, spec) = match source {
        Source::Activity => (
            activity(),
            "pid:text,state:text,wait:text,age_ms:ms,transaction_ms:ms,blockers:text,query:query,user:text,database:text,application:text,query_start:text",
        ),
        Source::Database => (
            format!(
                "SELECT datid::text, numbackends, xact_commit, xact_rollback, blks_hit, blks_read, temp_files, temp_bytes, deadlocks, tup_returned, tup_fetched, tup_inserted, tup_updated, tup_deleted, {DB_EPOCH} FROM pg_stat_database WHERE datname = current_database() LIMIT 1"
            ),
            "database_id:text,backends:count,xact_commit:counter,xact_rollback:counter,blks_hit:counter,blks_read:counter,temp_files:counter,temp_bytes:counter,deadlocks:counter,tup_returned:counter,tup_fetched:counter,tup_inserted:counter,tup_updated:counter,tup_deleted:counter",
        ),
        Source::Tables => (
            format!(
                "SELECT relid::text, schemaname, relname, seq_scan, idx_scan, n_live_tup, n_dead_tup, last_autovacuum::text, last_autoanalyze::text, last_vacuum::text, last_analyze::text, vacuum_count, autovacuum_count, analyze_count, autoanalyze_count, pg_relation_size(relid), (SELECT reltuples FROM pg_class WHERE oid = relid), n_mod_since_analyze, extract(epoch FROM clock_timestamp() - GREATEST(last_analyze, last_autoanalyze)) * 1000, extract(epoch FROM clock_timestamp() - GREATEST(last_vacuum, last_autovacuum)) * 1000, {DB_EPOCH} FROM pg_stat_user_tables ORDER BY n_dead_tup DESC, relid LIMIT 1000"
            ),
            "relation_id:text,schema:text,table:text,seq_scan:counter,idx_scan:counter,live_tuples:count,dead_tuples:count,last_autovacuum:text,last_autoanalyze:text,last_vacuum:text,last_analyze:text,vacuum_count:counter,autovacuum_count:counter,analyze_count:counter,autoanalyze_count:counter,size_bytes:bytes,estimated_rows:count,modifications:count,analyze_age_ms:ms,vacuum_age_ms:ms",
        ),
        Source::Indexes => (
            indexes(),
            "index_id:text,schema:text,table:text,index:text,idx_scan:counter,idx_tup_read:counter,idx_tup_fetch:counter,size_bytes:bytes,relation_id:text,key_columns:text,definition_key:text,usable:text,protected:text,reset_age_ms:ms",
        ),
        Source::Replication => (
            format!(
                "SELECT 'sender:' || pid, state, sync_state, extract(epoch FROM write_lag) * 1000, extract(epoch FROM flush_lag) * 1000, extract(epoch FROM replay_lag) * 1000, {EPOCH} FROM pg_stat_replication UNION ALL SELECT 'receiver:' || pid, status, NULL::text, NULL::numeric, NULL::numeric, NULL::numeric, {EPOCH} FROM pg_stat_wal_receiver LIMIT 1000"
            ),
            "process_id:text,state:text,sync_state:text,write_lag_ms:ms,flush_lag_ms:ms,replay_lag_ms:ms",
        ),
        Source::Statements => return statements(config),
        Source::Io => return Some(io(config)),
    };
    Some((sql, columns(spec)))
}

fn indexes() -> String {
    format!(
        "SELECT s.indexrelid::text, s.schemaname, s.relname, s.indexrelname, s.idx_scan, s.idx_tup_read, s.idx_tup_fetch, pg_relation_size(s.indexrelid), s.relid::text, array_to_json(ARRAY(SELECT a.attname FROM unnest(i.indkey::smallint[]) WITH ORDINALITY k(attnum, pos) JOIN pg_attribute a ON a.attrelid = s.relid AND a.attnum = k.attnum WHERE pos <= i.indnkeyatts ORDER BY pos))::text, md5(i.indkey::text || '|' || i.indcollation::text || '|' || i.indclass::text || '|' || i.indoption::text || '|' || COALESCE(i.indexprs::text, '') || '|' || COALESCE(i.indpred::text, '') || '|' || i.indisunique::text || '|' || c.relam::text || '|' || i.indnkeyatts::text), (i.indisvalid AND i.indisready AND i.indpred IS NULL AND i.indexprs IS NULL AND am.amname = 'btree'), (i.indisunique OR i.indisprimary OR i.indisreplident OR EXISTS(SELECT 1 FROM pg_constraint WHERE conindid = s.indexrelid)), extract(epoch FROM clock_timestamp() - (SELECT stats_reset FROM pg_stat_database WHERE datname = current_database())) * 1000, {DB_EPOCH} FROM pg_stat_user_indexes s JOIN pg_index i ON i.indexrelid = s.indexrelid JOIN pg_class c ON c.oid = s.indexrelid JOIN pg_am am ON am.oid = c.relam ORDER BY s.indexrelid LIMIT 1000"
    )
}

fn activity() -> String {
    format!(
        "WITH waiting AS (SELECT pid FROM pg_stat_activity WHERE wait_event_type = 'Lock' AND pid <> pg_backend_pid() ORDER BY query_start LIMIT 100) SELECT a.pid::text, a.state, {}, {}, GREATEST(0, extract(epoch FROM clock_timestamp() - a.xact_start) * 1000), CASE WHEN a.pid IN (SELECT pid FROM waiting) THEN pg_blocking_pids(a.pid)::text ELSE NULL END, CASE WHEN octet_length(a.query) < LEAST(4096, (SELECT setting::int FROM pg_settings WHERE name = 'track_activity_query_size') - 1) AND current_setting('standard_conforming_strings') = 'on' THEN a.query ELSE NULL END, a.usename, a.datname, a.application_name, a.query_start::text, {EPOCH} FROM pg_stat_activity a WHERE a.backend_type = 'client backend' AND a.pid <> pg_backend_pid() AND a.application_name NOT LIKE '% monitor' ORDER BY 4 DESC, a.pid LIMIT 1000",
        crate::admin::ACTIVITY_WAIT,
        crate::admin::ACTIVITY_AGE
    )
}

fn statements(config: &Config) -> Option<(String, Vec<Column>)> {
    let schema = config.schema.as_ref()?;
    let schema = format!("\"{}\"", schema.replace('"', "\"\""));
    let modern = config.columns.iter().any(|name| name == "total_exec_time");
    let (total, mean, max) = if modern {
        ("total_exec_time", "mean_exec_time", "max_exec_time")
    } else {
        ("total_time", "mean_time", "max_time")
    };
    let epoch = if config.statement_epoch {
        format!(
            "{EPOCH} || COALESCE((SELECT stats_reset::text FROM {schema}.pg_stat_statements_info), '')"
        )
    } else {
        EPOCH.to_owned()
    };
    let sql = format!(
        "SELECT dbid::text || ':' || userid::text || ':' || queryid::text, calls, {total}, {mean}, {max}, rows, shared_blks_hit, shared_blks_read, shared_blks_dirtied, shared_blks_written, temp_blks_read, temp_blks_written, CASE WHEN octet_length(query) < 4096 AND current_setting('standard_conforming_strings') = 'on' THEN query ELSE NULL END, {epoch} FROM {schema}.pg_stat_statements(true) WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database()) AND queryid IS NOT NULL ORDER BY {total} DESC, queryid LIMIT 200"
    );
    Some((
        sql,
        columns(
            "statement_id:text,calls:counter,total_ms:counter,cumulative_mean_ms:ms,max_since_reset_ms:ms,rows:counter,shared_blks_hit:counter,shared_blks_read:counter,shared_blks_dirtied:counter,shared_blks_written:counter,temp_blks_read:counter,temp_blks_written:counter,query:query",
        ),
    ))
}

fn columns(spec: &str) -> Vec<Column> {
    spec.split(',')
        .filter_map(|part| part.split_once(':'))
        .map(|(name, unit)| Column {
            name: name.to_owned(),
            unit: if unit == "counter" {
                if name.ends_with("bytes") {
                    "bytes"
                } else if name.ends_with("ms") {
                    "ms"
                } else {
                    "count"
                }
            } else {
                unit
            }
            .to_owned(),
            kind: match unit {
                "counter" => MetricKind::Counter,
                "text" => MetricKind::Text,
                "query" => MetricKind::Query,
                _ => MetricKind::Gauge,
            },
        })
        .collect()
}

fn io(config: &Config) -> (String, Vec<Column>) {
    let bytes = if config.io_bytes {
        "read_bytes, write_bytes"
    } else {
        "reads * op_bytes, writes * op_bytes"
    };
    let sql = format!(
        "SELECT backend_type || ':' || object || ':' || context, reads, writes, hits, evictions, reuses, fsyncs, {bytes}, {EPOCH} || COALESCE(stats_reset::text, '') FROM pg_stat_io ORDER BY backend_type, object, context LIMIT 1000"
    );
    (
        sql,
        columns(
            "io_identity:text,reads:counter,writes:counter,hits:counter,evictions:counter,reuses:counter,fsyncs:counter,read_bytes:counter,write_bytes:counter",
        ),
    )
}
