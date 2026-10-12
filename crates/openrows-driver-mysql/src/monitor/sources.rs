// SPDX-License-Identifier: Apache-2.0
//! Fixed projections and units; only bounded statistics views are sampled.
use super::{
    Config,
    layout::{Layout, columns},
};
use openrows_monitor::{Column, Source};

pub(super) fn uptime(layout: Layout) -> String {
    format!(
        "(SELECT VARIABLE_VALUE FROM {} WHERE VARIABLE_NAME = 'Uptime')",
        layout.status()
    )
}
pub(super) fn query(source: Source, config: &Config) -> (String, Vec<Column>) {
    let epoch = format!("{}, {}", tracking(source), uptime(config.layout));
    let (sql, spec) = match source {
        Source::Activity => (
            super::activity::query(config),
            "pid:text,state:text,wait:text,age_ms:ms,transaction_ms:ms,blockers:text,query:query,user:text,database:text,server_state:text,blocking_edges:count",
        ),
        Source::Database => (
            if config.probed.deadlocks {
                super::status::mysql()
            } else {
                "SHOW GLOBAL STATUS".into()
            },
            "",
        ),
        Source::Statements => (
            format!(
                "SELECT CONCAT(COALESCE(SCHEMA_NAME, ''), ':', DIGEST, ':', FIRST_SEEN), COUNT_STAR, SUM_TIMER_WAIT / 1000000000, AVG_TIMER_WAIT / 1000000000, MAX_TIMER_WAIT / 1000000000, SUM_ROWS_SENT, SUM_ROWS_EXAMINED, SUM_CREATED_TMP_DISK_TABLES, SUM_NO_INDEX_USED, CASE WHEN OCTET_LENGTH(DIGEST_TEXT) < 4096 THEN DIGEST_TEXT ELSE NULL END, {epoch} FROM performance_schema.events_statements_summary_by_digest WHERE SCHEMA_NAME = DATABASE() AND DIGEST IS NOT NULL ORDER BY SUM_TIMER_WAIT DESC, DIGEST LIMIT 200"
            ),
            "statement_id:text,calls:counter,total_ms:counter,cumulative_mean_ms:ms,max_since_reset_ms:ms,rows:counter,rows_examined:counter,temp_disk_tables:counter,no_index_used:counter,query:query",
        ),
        Source::Tables => (
            format!(
                "SELECT CONCAT(OBJECT_SCHEMA, '.', OBJECT_NAME), OBJECT_SCHEMA, OBJECT_NAME, COUNT_READ, COUNT_WRITE, SUM_TIMER_WAIT / 1000000000, {epoch} FROM performance_schema.table_io_waits_summary_by_table WHERE OBJECT_SCHEMA = DATABASE() ORDER BY SUM_TIMER_WAIT DESC, OBJECT_NAME LIMIT 1000"
            ),
            "relation_id:text,schema:text,table:text,reads:counter,writes:counter,total_ms:counter",
        ),
        Source::Indexes => (
            indexes(config, &epoch),
            "index_id:text,schema:text,table:text,index:text,reads:counter,writes:counter,total_ms:counter,unused_observed:text,redundant_to:text",
        ),
        Source::Replication => (
            (if config.layout.modern_process {
                "SHOW REPLICA STATUS"
            } else {
                "SHOW SLAVE STATUS"
            })
            .into(),
            "",
        ),
        Source::Io => (
            format!(
                "SELECT EVENT_NAME, COUNT_READ, COUNT_WRITE, SUM_NUMBER_OF_BYTES_READ, SUM_NUMBER_OF_BYTES_WRITE, SUM_TIMER_WAIT / 1000000000, {epoch} FROM performance_schema.file_summary_by_event_name ORDER BY SUM_TIMER_WAIT DESC, EVENT_NAME LIMIT 1000"
            ),
            "io_identity:text,reads:counter,writes:counter,read_bytes:counter,write_bytes:counter,total_ms:counter",
        ),
    };
    (sql, columns(spec))
}
fn indexes(config: &Config, epoch: &str) -> String {
    let (joins, health) = if config.probed.sys_indexes {
        (
            "LEFT JOIN sys.schema_unused_indexes u ON u.object_schema = i.OBJECT_SCHEMA AND u.object_name = i.OBJECT_NAME AND u.index_name = i.INDEX_NAME LEFT JOIN sys.schema_redundant_indexes r ON r.table_schema = i.OBJECT_SCHEMA AND r.table_name = i.OBJECT_NAME AND r.redundant_index_name = i.INDEX_NAME",
            "CASE WHEN u.index_name IS NULL THEN 'no' ELSE 'yes' END, r.dominant_index_name",
        )
    } else {
        ("", "NULL, NULL")
    };
    format!(
        "SELECT CONCAT(i.OBJECT_SCHEMA, '.', i.OBJECT_NAME, ':', COALESCE(i.INDEX_NAME, '(none)')), i.OBJECT_SCHEMA, i.OBJECT_NAME, i.INDEX_NAME, i.COUNT_READ, i.COUNT_WRITE, i.SUM_TIMER_WAIT / 1000000000, {health}, {epoch} FROM performance_schema.table_io_waits_summary_by_index_usage i {joins} WHERE i.OBJECT_SCHEMA = DATABASE() ORDER BY i.SUM_TIMER_WAIT DESC, i.OBJECT_NAME, i.INDEX_NAME LIMIT 1000"
    )
}

fn tracking(source: Source) -> String {
    let pattern = match source {
        Source::Statements => "statement/%",
        Source::Tables | Source::Indexes => "wait/io/table/%",
        Source::Io => "wait/io/file/%",
        Source::Activity | Source::Database | Source::Replication => return "1".into(),
    };
    let instruments = format!(
        "EXISTS(SELECT 1 FROM performance_schema.setup_instruments WHERE NAME LIKE '{pattern}' AND ENABLED = 'YES' AND TIMED = 'YES')"
    );
    if source == Source::Statements {
        format!(
            "({instruments} AND (SELECT COUNT(*) FROM performance_schema.setup_consumers WHERE NAME IN ('global_instrumentation', 'thread_instrumentation', 'statements_digest') AND ENABLED = 'YES') = 3)"
        )
    } else {
        format!(
            "({instruments} AND (SELECT COUNT(*) FROM performance_schema.setup_consumers WHERE NAME IN ('global_instrumentation', 'thread_instrumentation') AND ENABLED = 'YES') = 2)"
        )
    }
}
