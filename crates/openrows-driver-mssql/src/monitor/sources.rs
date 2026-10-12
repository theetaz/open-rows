// SPDX-License-Identifier: Apache-2.0
//! Fixed DMV projections and units; every projection ends with `@epoch`.
use openrows_monitor::{Column, MetricKind, Source};

use super::platform::Platform;

const ACTIVITY: &str = "SELECT TOP (1000) CAST(s.session_id AS varchar(11)), CASE WHEN r.session_id IS NOT NULL THEN 'active' WHEN s.open_transaction_count > 0 THEN 'idle in transaction' ELSE 'idle' END, r.wait_type, CASE WHEN COALESCE(r.start_time, s.last_request_start_time) >= SYSDATETIME() THEN 0 ELSE DATEDIFF_BIG(millisecond, COALESCE(r.start_time, s.last_request_start_time), SYSDATETIME()) END, CASE WHEN x.began IS NULL THEN NULL WHEN x.began >= GETDATE() THEN 0 ELSE DATEDIFF_BIG(millisecond, x.began, GETDATE()) END, CASE WHEN r.blocking_session_id > 0 THEN CAST(r.blocking_session_id AS varchar(11)) END, CASE WHEN DATALENGTH(t.text) < 4096 THEN t.text END, s.login_name, DB_NAME(COALESCE(r.database_id, s.database_id)), COALESCE(r.command, s.status), CASE WHEN r.blocking_session_id > 0 THEN 1 ELSE 0 END, @epoch FROM sys.dm_exec_sessions AS s LEFT JOIN sys.dm_exec_requests AS r ON r.session_id = s.session_id LEFT JOIN sys.dm_exec_connections AS c ON c.session_id = s.session_id AND c.net_transport <> N'Session' OUTER APPLY sys.dm_exec_sql_text(COALESCE(r.sql_handle, c.most_recent_sql_handle)) AS t OUTER APPLY (SELECT MIN(a.transaction_begin_time) AS began FROM sys.dm_tran_session_transactions AS st JOIN sys.dm_tran_active_transactions AS a ON a.transaction_id = st.transaction_id WHERE st.session_id = s.session_id) AS x WHERE s.is_user_process = 1 AND s.session_id <> @@SPID ORDER BY 4 DESC, s.session_id";

/// Top 200 query-hash groups whose plans compiled in this database (ad hoc
/// text has no `dbid`); text comes from the most recently executed member.
const STATEMENTS: &str = "WITH s AS (SELECT q.query_hash, q.sql_handle, q.statement_start_offset, q.statement_end_offset, q.execution_count, q.total_elapsed_time, q.max_elapsed_time, q.total_rows, q.total_worker_time, q.total_logical_reads, q.total_physical_reads, q.total_logical_writes, ROW_NUMBER() OVER (PARTITION BY q.query_hash ORDER BY q.last_execution_time DESC) AS n FROM sys.dm_exec_query_stats AS q CROSS APPLY sys.dm_exec_plan_attributes(q.plan_handle) AS a WHERE a.attribute = N'dbid' AND CONVERT(int, a.value) = DB_ID()), g AS (SELECT TOP (200) query_hash, SUM(execution_count) AS calls, SUM(total_elapsed_time) / 1000.0 AS total_ms, MAX(max_elapsed_time) / 1000.0 AS max_ms, SUM(total_rows) AS row_count, SUM(total_worker_time) / 1000.0 AS cpu_ms, SUM(total_logical_reads) AS logical_reads, SUM(total_physical_reads) AS physical_reads, SUM(total_logical_writes) AS logical_writes, MAX(CASE WHEN n = 1 THEN sql_handle END) AS handle, MAX(CASE WHEN n = 1 THEN statement_start_offset END) AS start_at, MAX(CASE WHEN n = 1 THEN statement_end_offset END) AS end_at FROM s GROUP BY query_hash ORDER BY SUM(total_elapsed_time) DESC, query_hash) SELECT CONVERT(varchar(20), g.query_hash, 1), g.calls, g.total_ms, g.total_ms / NULLIF(g.calls, 0), g.max_ms, g.row_count, g.cpu_ms, g.logical_reads, g.physical_reads, g.logical_writes, CASE WHEN DATALENGTH(x.text) < 4096 THEN x.text END, @epoch FROM g OUTER APPLY (SELECT SUBSTRING(t.text, g.start_at / 2 + 1, (CASE WHEN g.end_at = -1 THEN DATALENGTH(t.text) ELSE g.end_at END - g.start_at) / 2 + 1) AS text FROM sys.dm_exec_sql_text(g.handle) AS t) AS x ORDER BY g.total_ms DESC, g.query_hash";

const TABLES: &str = "SELECT TOP (1000) CONCAT(SCHEMA_NAME(o.schema_id), '.', o.name), SCHEMA_NAME(o.schema_id), o.name, SUM(u.user_seeks), SUM(u.user_scans), SUM(u.user_lookups), SUM(u.user_updates), @epoch FROM sys.dm_db_index_usage_stats AS u JOIN sys.objects AS o ON o.object_id = u.object_id WHERE u.database_id = DB_ID() AND o.is_ms_shipped = 0 GROUP BY o.schema_id, o.name ORDER BY SUM(u.user_seeks + u.user_scans + u.user_lookups + u.user_updates) DESC, o.name";

const INDEXES: &str = "SELECT TOP (1000) CONCAT(SCHEMA_NAME(o.schema_id), '.', o.name, ':', COALESCE(i.name, '(heap)')), SCHEMA_NAME(o.schema_id), o.name, COALESCE(i.name, '(heap)'), u.user_seeks, u.user_scans, u.user_lookups, u.user_updates, @epoch FROM sys.dm_db_index_usage_stats AS u JOIN sys.objects AS o ON o.object_id = u.object_id JOIN sys.indexes AS i ON i.object_id = u.object_id AND i.index_id = u.index_id WHERE u.database_id = DB_ID() AND o.is_ms_shipped = 0 ORDER BY u.user_seeks + u.user_scans + u.user_lookups + u.user_updates DESC, o.name, i.index_id";

/// Idle/background waits that never indicate workload pressure.
const BENIGN: &str = "wait_type NOT LIKE N'SLEEP[_]%' AND wait_type NOT LIKE N'BROKER[_]%' AND wait_type NOT LIKE N'XE[_]%' AND wait_type NOT LIKE N'SQLTRACE[_]%' AND wait_type NOT LIKE N'QDS[_]%' AND wait_type NOT LIKE N'PREEMPTIVE[_]XE[_]%' AND wait_type NOT IN (N'CHECKPOINT_QUEUE', N'CHKPT', N'CLR_AUTO_EVENT', N'CLR_MANUAL_EVENT', N'CLR_SEMAPHORE', N'DBMIRROR_EVENTS_QUEUE', N'DBMIRRORING_CMD', N'DIRTY_PAGE_POLL', N'DISPATCHER_QUEUE_SEMAPHORE', N'FT_IFTS_SCHEDULER_IDLE_WAIT', N'FT_IFTSHC_MUTEX', N'HADR_CLUSAPI_CALL', N'HADR_FILESTREAM_IOMGR_IOCOMPLETION', N'HADR_LOGCAPTURE_WAIT', N'HADR_NOTIFICATION_DEQUEUE', N'HADR_TIMER_TASK', N'HADR_WORK_QUEUE', N'KSOURCE_WAKEUP', N'LAZYWRITER_SLEEP', N'LOGMGR_QUEUE', N'ONDEMAND_TASK_QUEUE', N'PARALLEL_REDO_WORKER_WAIT_WORK', N'PWAIT_ALL_COMPONENTS_INITIALIZED', N'PWAIT_EXTENSIBILITY_CLEANUP_TASK', N'REQUEST_FOR_DEADLOCK_SEARCH', N'RESOURCE_QUEUE', N'SERVER_IDLE_CHECK', N'SOS_WORK_DISPATCHER', N'SP_SERVER_DIAGNOSTICS_SLEEP', N'UCS_SESSION_REGISTRATION', N'WAIT_XTP_CKPT_CLOSE', N'WAIT_XTP_OFFLINE_CKPT_NEW_LOG', N'WAITFOR', N'XTP_PREEMPTIVE_TASK')";

/// The fixed batch (epoch declaration plus projection) and column contract.
pub(super) fn query(source: Source, platform: Platform) -> Option<(String, Vec<Column>)> {
    let (sql, spec) = match source {
        Source::Activity => (
            ACTIVITY.to_owned(),
            "pid:text,state:text,wait:text,age_ms:ms,transaction_ms:ms,blockers:text,query:query,user:text,database:text,server_state:text,blocking_edges:count".to_owned(),
        ),
        Source::Database => (super::counters::query(), super::counters::spec()),
        Source::Statements => (
            STATEMENTS.to_owned(),
            "statement_id:text,calls:counter,total_ms:counter,cumulative_mean_ms:ms,max_since_reset_ms:ms,rows:counter,cpu_ms:counter,logical_reads:counter,physical_reads:counter,logical_writes:counter,query:query".to_owned(),
        ),
        Source::Tables => (
            TABLES.to_owned(),
            "relation_id:text,schema:text,table:text,seeks:counter,scans:counter,lookups:counter,updates:counter".to_owned(),
        ),
        Source::Indexes => (
            INDEXES.to_owned(),
            "index_id:text,schema:text,table:text,index:text,seeks:counter,scans:counter,lookups:counter,updates:counter".to_owned(),
        ),
        Source::Io => (
            format!(
                "SELECT TOP (200) wait_type, waiting_tasks_count, wait_time_ms, signal_wait_time_ms, max_wait_time_ms, @epoch FROM {} WHERE waiting_tasks_count > 0 AND {BENIGN} ORDER BY wait_time_ms DESC, wait_type",
                platform.waits()
            ),
            "wait_type:text,waiting_tasks:counter,wait_ms:counter,signal_wait_ms:counter,max_wait_ms:ms".to_owned(),
        ),
        Source::Replication => return None,
    };
    Some((format!("{} {sql}", platform.epoch()), columns(&spec)))
}

pub(super) fn columns(spec: &str) -> Vec<Column> {
    spec.split(',')
        .filter_map(|part| part.split_once(':'))
        .map(|(name, unit)| Column {
            name: name.into(),
            unit: match unit {
                "counter" if name.ends_with("ms") => "ms",
                "counter" => "count",
                other => other,
            }
            .into(),
            kind: match unit {
                "counter" => MetricKind::Counter,
                "text" => MetricKind::Text,
                "query" => MetricKind::Query,
                _ => MetricKind::Gauge,
            },
        })
        .collect()
}
