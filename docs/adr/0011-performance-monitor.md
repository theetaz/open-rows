# 0011 — Database Performance Monitor

- Status: Accepted 2026-10-11 (owner) — validation pending (M25-09)
- Date: 2026-10-11
- Deciders: OpenRows maintainer (owner)
- Requirements: FR-ADMIN-03..08 (P1; M2.5), FR-AI-11..13 (M4.5 integration), NFR-PERF-11, NFR-SEC-08; FR-ADMIN-01, FR-EDIT-07, FR-APP-11, NFR-REL-02/04, NFR-UX-01..03
- Related: [ADR-0002](0002-arrow-ipc-for-results.md), [ADR-0007](0007-session-actor-model.md), [ADR-0010](0010-driver-host-process.md), [docs/13](../13-performance.md), [M2.5 tasks](../27-delivery-plan.md#5a-m25--database-monitoring-and-analytics), [product scope](../28-database-monitoring-and-ai-harness.md)

## Context

A connected database also serves other applications. Its operator needs to see current work, expensive completed queries, resource pressure and evidence for possible improvements. Timing only OpenRows queries cannot describe that workload. Polling activity alone misses queries that finish between samples; cumulative statistics and bounded event history complement it. This is sampled monitoring, not a lossless query trace or an operating-system profiler.

### Current implementation

| Boundary | Verified source and implication |
| --- | --- |
| Process list (#77; FR-ADMIN-01; docs/26 A01–A03; docs/27 M2-15) | [`ProcessesTab.tsx`](../../apps/desktop/src/features/processes/ProcessesTab.tsx), [`processModel.ts`](../../apps/desktop/src/features/processes/processModel.ts) and [`useProcesses.ts`](../../apps/desktop/src/features/processes/useProcesses.ts): filtering/sorting, off/2/5/10 s refresh (5 s default), no overlapping listing, metadata connection. [`processSignals.ts`](../../apps/desktop/src/features/processes/processSignals.ts) supplies capability-gated cancel/kill. Reuse presentation and guarded actions; its current raw-query DTO is not the monitor's privacy contract. |
| Explain (#86, #96; FR-EDIT-07; M2-06) | [`ExplainView.tsx`](../../apps/desktop/src/features/explain/ExplainView.tsx) and [`PlanFlame.tsx`](../../apps/desktop/src/features/explain/PlanFlame.tsx): tree, hotspots, raw and flame views. [`openrows-explain`](../../crates/openrows-explain/src/command.rs) supports PG/MySQL/MariaDB/SQLite/SQL Server plans; ClickHouse has no equivalent visualiser contract. [`explain/admit.rs`](../../crates/openrows-core/src/explain/admit.rs) currently permits read ANALYZE without confirmation under some policies; this decision requires confirmation for every monitor-initiated measured execution. |
| Scope and safety | [docs/02](../02-requirements.md#17-administration-fr-admin), [docs/19 M2.5](../19-roadmap.md#m25--database-monitoring-and-analytics) and [docs/26 G07](../26-feature-parity.md#deferred-excluded-and-bounded-alternatives) now plan monitoring and analytics; [docs/28](../28-database-monitoring-and-ai-harness.md) owns the expanded product scope. No slow-query monitor, server resource/memory view or optimisation advisor is implemented. [`safety.rs`](../../crates/openrows-core/src/safety.rs), [`safety/policies.rs`](../../crates/openrows-core/src/safety/policies.rs) and [docs/12](../12-security.md#implemented-m2-22a-connection-safety-and-application-locks) govern proposals and execution (#99). |
| Engines | [docs/05 matrix](../05-database-drivers.md#4-per-database-strategy): PostgreSQL, MySQL/MariaDB, SQLite, SQL Server, ClickHouse and libSQL (remote/file). DuckDB is hosted work under repair in M2-04a; [ADR-0010](0010-driver-host-process.md#duckdb-file-ownership-and-session-allocation) forbids a second hidden connection for a writable file. Redis and other NoSQL sources need a later specification. |

## Decision

1. Add a **Monitor** workspace tab per connection, with explicit database/node and visibility scope. Use engine-owned statistics to observe other applications where permitted. Display unsupported, disabled, restricted and stale sources separately from valid zero measurements.
2. Keep collection, capabilities, counter semantics, sampling and deterministic advice in Tauri-free `crates/`: a planned `openrows-monitor` orchestration/analysis crate, source implementations in driver crates, actor/session ownership in `openrows-core`. The desktop supplies thin typed commands/events, profile policy and UI only. The M25-01/02 backend slice is recorded in the implementation evidence below; later product and engine work remains planned.
3. For server engines, own a dedicated read-only monitoring session per connection workspace; never borrow the user's query/transaction session or the existing metadata session. Share one collector between windows/tabs for the same profile, endpoint and credential scope. Switching databases re-probes database-scoped sources, ends the old generation and never merges unrelated scopes. Additional database collectors count against the global limits and require explicit selection.
4. Embedded engines use **own-session statistics only**: capture counters/profile results through the owning actor and publish observations without a second SQL session. This exception preserves ADR-0010. Do not queue periodic SQL behind a busy embedded query and call it live sampling; show “Updated after query completion”.
5. Default to bounded memory and no persistence. Default query display is redacted normalised/digest text. Advisor output is a proposal; no collector, rule or refresh executes user SQL, DDL, ANALYZE, VACUUM, instrumentation changes or statistics resets.
6. Deliver monitoring and analytics at **M2.5 (P1)** immediately after the M2 DuckDB host repair, PostgreSQL first, then the other engines. [docs/28](../28-database-monitoring-and-ai-harness.md) owns charts, boards, alerts and reports; this ADR owns the source, privilege, budget, privacy and advisor contracts. The optional database AI harness follows at **M4.5**, under ADR-0009 and its separately reviewed sharing/tool boundaries. M25-01/02 are partial backend deliveries; docs/27 records the remaining validation.

### Data sources and permissions

The minima below are accepted monitor support floors, aligned to docs/05, rather than claims that every view first appeared in those releases. Probe product/version, objects, columns, settings and effective read access before preparing source SQL; PG-compatible services, MySQL-compatible flavours and managed services must pass the same probes. Newer columns are optional. A denied probe remains “Permission required”, not “Unsupported”. Vendor references at the end support source names, prerequisites and limitations.

#### PostgreSQL (12+; PG 16+ I/O extension to the baseline)

| Source | Reads / meaning | Prerequisites and unavailable data |
| --- | --- | --- |
| `pg_stat_activity`; `pg_locks`; `pg_blocking_pids(pid)` | Session states, query age, transaction age, waits and bounded blocking edges. Reuse process-list columns, excluding the monitor PID. | `CONNECT` to the selected database; own sessions visible by default, `pg_read_all_stats` for other roles' restricted fields. `track_activities` must be on for query activity. Lock snapshots race; the blocking function briefly accesses shared lock-manager state, so cap calls at 100 waiting PIDs/sample. No general per-query memory or CPU metric. [PG1], [PG4] |
| `pg_stat_statements` / `pg_stat_statements(false)` | Calls, rows, execution time, shared/local/temp blocks, I/O time. PG 12 uses `total_time`/`mean_time`/`max_time`; PG 13+ uses `total_exec_time`/`mean_exec_time`/`max_exec_time` and optional planning statistics. | Installed extension in this database, module preloaded and tracking enabled. `SELECT`/function access; `pg_read_all_stats` for other users' text/query IDs. `track_io_timing` required for meaningful I/O timings; disabled timings are unavailable, not zero. No per-query memory. Prefer `showtext=false` on repeated samples, fetch/redact text only on explicit detail requests. [PG2], [PG3] |
| `pg_stat_database`; `pg_stat_user_tables`; `pg_stat_user_indexes`; `pg_statio_user_tables`/`pg_statio_user_indexes` | Database cache hits/reads, transactions, temp files/bytes; table/index scans, dead/live tuple estimates, last (auto)vacuum/analyse and maintenance counts. | `track_counts` for table/index counters. Selected database for relation views. Standard statistics visibility; restricted fields still need `pg_read_all_stats`. Dead-tuple estimates/maintenance age are vacuum/bloat indicators, not measured bloat percentages; no automatic `pgstattuple` scan or extension installation. [PG1] |
| `pg_stat_io` (16+); `pg_stat_replication` | I/O by backend/object/context, not query; replication write/flush/replay lag where exposed. | Probe columns and `track_io_timing`; cross-session replication detail needs statistics visibility. Below PG 16, I/O falls back to database/statement block counters. Lag is nullable and not a prediction of catch-up time; no replicas means “Not applicable”. [PG1], [PG5] |

**Extension setup flow:** read `pg_extension`/`pg_available_extensions`, accessible `shared_preload_libraries`, `pg_stat_statements.track`, and on PG 14+ `compute_query_id` (`auto`/`on` or a compatible query-ID provider). Probe the view itself: installed does not mean loaded/usable. Distinguish unavailable package, installed-but-not-preloaded, not created in this database, disabled tracking and denied access. Offer “Show setup instructions” and editable `CREATE EXTENSION pg_stat_statements` only; explain DBA/provider configuration, preload restart and overhead. Installation is normally privileged (or provider-controlled); never request superuser for routine monitoring, run `ALTER SYSTEM` or restart the server. The owner accepted guidance plus reviewed editor SQL only; enabling remains an explicit user action under the safety policy. [PG2], [PG6]

#### MySQL (5.7/8.0/8.4) and MariaDB (10.6+)

| Source | Reads / meaning | Prerequisites and differences |
| --- | --- | --- |
| `performance_schema.events_statements_summary_by_digest`; `events_statements_current`; `events_statements_history` (optionally `_history_long`); `threads` | Digest/schema, calls, total/mean/max latency, rows examined/sent, lock time, temporary disk-table counts and scan/sort indicators; sampled current/recent work. Timers are picoseconds, converted once to ms. | `performance_schema=ON` at server start; enabled `statement/%` instruments, timing and relevant consumers (`statements_digest`, current/history plus their parent consumers). Grant `SELECT` on the required Performance Schema tables; no setup-table `UPDATE` is needed to read. History is small/overwritten and cannot promise every query. MariaDB is often off by default; inspect settings rather than infer availability from an empty table. [MY1], [MY2], [MY3], [MY11], [MA1], [MA2] |
| `sys.statement_analysis` / `sys.x$statement_analysis`; `sys.schema_unused_indexes`; `sys.schema_redundant_indexes` | Optional convenience views for statement and index advice; prefer base tables/numeric `x$` views for stable units. | MySQL 5.7+ ships `sys`; MariaDB introduced it in 10.6. Probe each view; layouts/functions differ. `SELECT` on chosen views/underlying tables and `EXECUTE` only on required read helpers; no access to modifying `sys` procedures. Missing `sys` does not disable base-table monitoring. [MY4], [MY13], [MA3], [MA4] |
| `performance_schema.memory_summary_global_by_event_name` / `memory_summary_by_thread_by_event_name` | Instrumented current/high-water allocations, summed by event or thread; never label as total server RSS. | Enabled `memory/%` instrumentation; MySQL 5.7 baseline, MariaDB memory tables from 10.5.2 (within the 10.6 floor). MySQL 8.0.31+ statement `MAX_CONTROLLED_MEMORY`/`MAX_TOTAL_MEMORY` are optional peak metrics; do not presume these columns on MariaDB. [MY1], [MY5], [MA5] |
| `information_schema.INNODB_METRICS`; `performance_schema.global_status`; `events_waits_summary_global_by_event_name`; `file_summary_by_event_name` | Enabled InnoDB counters, buffer-pool read requests/reads, connections, wait and file-I/O counters. | `SELECT` on Performance Schema sources; `PROCESS` for `INNODB_METRICS`. Check each metric's enabled status; never enable `innodb_monitor_enable` automatically. Cache hit ratio describes the InnoDB buffer pool, not OS caching. [MY1], [MY6], [MY7], [MA6] |
| `information_schema.INNODB_TRX`; MySQL 8.0 `performance_schema.data_locks`/`data_lock_waits`; MySQL 5.7 and MariaDB `information_schema.INNODB_LOCKS`/`INNODB_LOCK_WAITS` | Transaction age, requested/held locks, blockers and blocked threads. | `PROCESS` for InnoDB Information Schema transaction/lock tables; `SELECT` for Performance Schema lock tables. MySQL replaced Information Schema lock tables in 8.0; MariaDB retains them. Omit `LOCK_DATA` and transaction SQL values from the monitor payload. [MY7], [MY8], [MA7] |
| MySQL `SHOW SLAVE STATUS` (5.7) / `SHOW REPLICA STATUS` (8.0.22+); MariaDB `SHOW REPLICA STATUS` | Optional replication lag/status. | MySQL `REPLICATION CLIENT`; MariaDB 10.6+ `REPLICA MONITOR`. Read named lag/status fields only; discard credentials/addresses. NULL lag, stopped applier and no replica are distinct states. [MY9], [MA8] |

#### SQL Server (2017+; Azure SQL capability-gated)

| Source | Reads / meaning | Required access and caveats |
| --- | --- | --- |
| `sys.dm_exec_requests`; `sys.dm_exec_sessions`; `sys.dm_exec_sql_text(sql_handle)` | Sessions/requests, elapsed/CPU time, reads/writes, waits, `blocking_session_id`, query text (redacted in core), granted query memory in 8 KiB pages. | SQL Server ≤ 2019: `VIEW SERVER STATE`; 2022+: `VIEW SERVER PERFORMANCE STATE` for instance visibility. Without access some views expose only own work. Azure SQL requests are current-connection scoped; sessions can be database-scoped with `VIEW DATABASE STATE`. Do not promise instance-wide activity in Azure SQL. Negative blocking IDs represent special/unknown owners, not real sessions. [MS1], [MS2], [MS3] |
| `sys.dm_exec_query_stats`; optional `sys.dm_exec_query_plan(plan_handle)` | Completed cached-plan counters: calls, total/max time, CPU, reads, rows, grants/spills where the column/version supports them. Plan generations, eviction and recompilation break continuity. | Same server-state/performance-state distinction; Azure SQL query-stat access depends on service tier (`VIEW DATABASE STATE` or `##MS_ServerStateReader##`/administrator on restricted tiers). Cached plans can contain literals: redact XML before IPC. Missing plans are unavailable, not zero cost. [MS3], [MS4] |
| `sys.database_query_store_options`; `sys.query_store_query`, `_query_text`, `_plan`, `_runtime_stats`, `_runtime_stats_interval`, `_wait_stats` | Database Query Store aggregates/history; wait statistics available since SQL Server 2017. Runtime rows group by plan, execution type and interval; combine multiple rows for the active interval before computing changes. | Query Store since SQL Server 2016 (within floor); enabled/capturing state and retention determine coverage. Read access: `VIEW DATABASE STATE`, or SQL Server 2022+ `VIEW DATABASE PERFORMANCE STATE`. Never enable Query Store or force plans automatically. Server-persisted history may exist even though OpenRows local history is off. [MS5], [MS6] |
| `sys.dm_os_wait_stats`; `sys.dm_os_memory_clerks`; `sys.dm_exec_query_memory_grants`; `sys.dm_io_virtual_file_stats(NULL,NULL)`; `sys.dm_os_performance_counters` | Completed waits, engine memory accounting, live workspace-memory grants, file I/O deltas and buffer-cache counters. Grants are not all query memory; clerk totals are not per-query attribution. | Server-state permissions for older SQL Server, server-performance-state for 2022+ as documented per DMV. Probe each separately; Azure SQL can restrict instance sources. Cache-hit counters require their base counter; never sum ratios. No host-wide memory/CPU profiler. [MS7], [MS8], [MS9], [MS10], [MS11] |
| `sys.dm_db_missing_index_details`; `sys.dm_db_missing_index_groups`; `sys.dm_db_missing_index_group_stats`; optional `sys.dm_hadr_database_replica_states` | Missing-index evidence (not instructions to blindly create an index); Always On send/redo queues and commit-time evidence. | Server-state/performance-state access; missing-index data is volatile and capped (600 rows). Replica DMV only where Always On/access is available; queued bytes and commit timestamps are not a universal seconds-lag value. [MS12], [MS13] |

#### ClickHouse (25.3 LTS support floor)

| Source | Reads / meaning | Required access and limits |
| --- | --- | --- |
| `system.processes` | Live queries: elapsed, read/written rows/bytes, `memory_usage`, optional `peak_memory_usage`, normalised hash and profile events. Memory tracking can omit some allocations. | Read access to the selected system table; probe actual visibility/row policy. No persistent-session states or OLTP blocking-chain equivalent is claimed. This is the connected node, not an automatic cluster scan. [CH1], [CH7] |
| `system.query_log` | Terminal `QueryFinish` / `ExceptionWhileProcessing` events: query ID, duration, rows/bytes, memory, `normalized_query_hash`, `ProfileEvents`. Exclude `QueryStart` to avoid double counting. | `SELECT` on query log; configured `query_log`, `log_queries` and capture/sampling settings. Log flushing adds lag; never call `SYSTEM FLUSH LOGS`. Use time predicate/partition pruning, bounded overlap and event-ID deduplication. Count `is_initial_query` separately from shard work. Log absence/sampling means incomplete coverage. [CH2], [CH7] |
| `system.metrics`; `system.events`; `system.asynchronous_metrics`; optional `system.replicas` | Instantaneous gauges, cumulative event counters, periodic memory/I/O gauges; replica queue/`absolute_delay` where exposed. | Read grants on each selected table; probe names and units instead of hard-coded assumptions across versions/cloud. Asynchronous refresh is server-owned. Replica delay is table/replica-specific; no replicas means not applicable. Server OS gauges, if exposed, retain their source/scope; no general cache-hit ratio is invented. [CH3], [CH4], [CH5], [CH6], [CH7] |

#### Embedded engines and remote libSQL

| Engine / support floor | Offered sources | Explicit boundary / permissions |
| --- | --- | --- |
| SQLite / libSQL file: SQLite 3.20.0+ for the listed statement-memory counter | Own execution wall time/rows; `sqlite3_stmt_status` (`FULLSCAN_STEP`, `SORT`, `VM_STEP`, `MEMUSED`), `sqlite3_db_status` (cache use/hit/miss); optional `sqlite3_stmt_scanstatus` with `SQLITE_ENABLE_STMT_SCANSTATUS`. | Linked library/API and compile-option probes; normal file permissions/read-only mode. `MEMUSED` is prepared-statement storage, not peak query memory. Do not enable unsupported compile options or claim file-wide sessions/locks/other-process queries. UI: **“Own OpenRows sessions only — other applications are not visible.”** [SQ1], [SQ2], [SQ3], [SQ4] |
| DuckDB: 1.4 LTS+ with function/metric probes | `duckdb_memory()` (`memory_usage_bytes`, `temporary_storage_bytes`); `duckdb_temporary_files()`; optional own-query profiling through the host API with `enable_profiling='no_output'`. Host-instance buffer-manager accounting and completed-query operator timings/rows are distinct. | File access/read-only ownership under ADR-0010; no extension/autoload required. Capture profile before subsequent SQL overwrites it, restore prior settings; no stdout/file profile by default. 1.5-only profiling helper functions are not assumed on 1.4. Never open another writable-file host/session; host-local queries cannot see other processes. Live memory during busy SQL is unavailable unless a future safe host API exposes it. [DU1], [DU2], [DU3], [DU4] |
| libSQL remote: existing Hrana 3 contract | Own client wall time/returned rows and execution outcomes; server timings only if a documented, probed response exposes them. | Existing database auth/read permissions; no management API token requested. Hrana is not a server-monitoring DMV API: other clients' slow queries, server memory and waits unavailable. Do not route through SQLite C APIs or scrape provider dashboards. UI: **“Own OpenRows requests only — server-wide metrics unavailable over Hrana.”** [LI1] |

### Architecture, sampling and overhead

| Boundary | Accepted contract / budget |
| --- | --- |
| Monitor admission | Explicit open/resume only; reuse profile routing/TLS/credential policy. Apply only monitor-owned safe session settings, never arbitrary profile startup SQL. Fixed, reviewed read-only source templates; values bound, identifiers from probed/catalog metadata and quoted. Engine read-only settings supplement this allowlist; SQL Server has no general session read-only switch. No user-supplied polling SQL, resets, temp tables, `FOR UPDATE`, maintenance or implicit EXPLAIN. Cancel/kill uses the existing guarded admin path outside the collector, with policy/lock rechecked. Losing privilege disables only that source. |
| Scheduling | Base interval 5 s; selector off/2/5/10/30 s. Live sources use base interval, digest/resource sources at least 15 s, relation/advisor/replication sources at least 60 s. One in-flight source at a time; ≤ 1 data query per due source/interval, no per-row text/plan fetch loop. Coalesce shared live-session refresh so the existing Processes tab does not double-poll. Discovery is separately capped at 16 small reads/open or explicit re-probe; never per tick. |
| Adaptation / failure | After three unchanged samples double each source interval to at most 60 s; changed data restores its floor. Slow reads (over 20% of their interval) also back off. Transient failures use 2× back-off up to 60 s and a visible last-success age; no immediate retries or catch-up queue. Pause hidden/background workspaces by default; pause on disconnect, app lock, OS suspension or last subscriber close. Resume starts a fresh delta baseline; host errors retain ADR-0010's explicit reconnect requirement. |
| Server impact | Default source deadline `min(1 s, interval × 20%)`; aggregate collector query time ≤ 20% of elapsed minute, otherwise throttle. No user-table scans or explicit locks. Statistics views can take internal locks: **no lock-free server guarantee**. PG: short `BEGIN READ ONLY`, `SET LOCAL statement_timeout` and `lock_timeout` (100 ms), read, commit; no transaction/snapshot spans ticks. MySQL: read-only transaction / `MAX_EXECUTION_TIME` for supported SELECT; MariaDB: `max_statement_time`; SQL Server: client cancellation plus `SET LOCK_TIMEOUT 100` (not a statement timeout); ClickHouse: `readonly=1`, `max_execution_time` and result/read caps with overflow set to throw, supplemented by client cancellation (server limits can overshoot between checks). Apply guards before entering read-only mode, respecting server settings constraints. Failure to apply a guard disables that source, never runs unbounded. [PG7], [MY10], [MA9], [MS14], [CH8], [CH9] |
| Scan/result limits | SQL `LIMIT`/`TOP` caps returned rows, not work done by aggregate sorts. Push database/time predicates; suppress a source whose scan cannot finish within deadline/caps. Explicit column projections (no `SELECT *`), bounded text and driver admission cap raw responses before allocation; truncated text fails closed to fingerprint only. ≤ 1,000 rows/source/sample, ≤ 1 MiB encoded and ≤ 4 MiB decoded in-flight sample, text ≤ 4 KiB after redaction. Truncation/eviction shows “Partial”; top-N is best among covered data, never asserted globally complete. Blocking chains ≤ 100 edges/depth 16, cycles/unknown owners visible. |
| Memory / retention | Per-monitor ring buffers: 10 min default/max live window, ≤ 300 timestamps/series, ≤ 64 scalar series, ≤ 200 query identities (top-N default 20, max 100). Coarsen longer windows to fit timestamp caps; bounded query aggregates, no query-by-metric unbounded series product. ≤ 16 MiB retained/monitor, ≤ 32 MiB retained/app, ≤ 4 active collectors/app; account strings, maps, Arrow, worker copies and in-flight buffers. Evict oldest data/least-recent identities; no spill when local history is off. |
| IPC / UI | Reuse Arrow storage/window IPC and worker decode per ADR-0002; small typed capability/status events, never stream raw SQL rows as JSON. Release samples as replaced; visible windows only; coalesce paints ≤ 1 Hz. Pause cancels/discards pending samples; generation IDs reject late replies after scope change/close/lock. Lazy-load the tab, including charts; no initial-bundle growth above docs/13's 350 KB gzip budget. |
| App performance | Monitor increment ≤ 32 MiB physical footprint (all app/webview/host processes), average CPU ≤ 1% of one core over 60 s on mac-low at default cadence with one collector; pure sample delta/top-N analysis p95 ≤ 10 ms for 1,000 rows; no UI task > 50 ms. Total connected idle footprint remains ≤ 200 MB (NFR-PERF-02); these are budgets, not measured results. Check multi-collector/global caps and unchanged docs/13 startup, input and result-paint budgets at M2.5 exit. |
| Optional local history | Off per connection by default. If enabled, retain only redacted aggregates/fingerprints, 24 h and 50 MiB/connection, 100 MiB/app hard cap; downsample rather than grow series. OS-private local store, no raw text/values/profile credentials, bounded writes, delete oldest first, Clear/history-disable purge, expiry on next startup too. Warn that object identifiers can still be sensitive and storage is not encrypted by default (docs/12 §6). No collector/settings state implicitly restores sampling after launch. |

### Slow-query analytics and resource view

Top queries sort by **window total time, window mean, source maximum, calls/rate, rows, I/O, temp/spill and memory** where supported. Retain units, source, database/node, observation window, collection delay, reset epoch and coverage on every metric. Rows examined, rows returned and affected rows are separate; temporary disk-table counts are not bytes spilled. Memory gauges, grant pages and execution peaks are not additive allocated-byte counters.

- **Counter deltas:** baseline the first sample; `delta = current − previous` only for the same source generation, identity, reset marker and monotonic counter. Rates use actual monotonic elapsed time; window mean is summed time deltas / summed call deltas, not a mean of means. No calls → mean unavailable. State/gauge values are sampled directly. Cache ratio = window hit deltas / (hit + read deltas), denominator zero → unavailable; MySQL buffer pool uses 1 − reads/read requests; SQL Server uses its documented numerator/base pair. These are engine-cache ratios, not physical-disk probabilities. [PG1], [MY6], [MS11]
- **Reset/rollover:** inspect `pg_stat_database.stats_reset`, `pg_stat_statements_info.stats_reset` where available, server uptime/generation, statement/plan generation and Query Store interval keys. A restart, explicit reset, negative delta, disappearance/reappearance, version/scope change or ambiguous rollover ends the series and starts a new baseline with a gap; never clamp a negative delta to zero or infer unsigned wrap. PG older extensions/digests without reset epochs can miss a reset followed by larger counters: label continuity best effort. Never call a reset function. Source maxima cannot be subtracted: label cumulative “Max since source reset/cache entry”; exact window max only from covered terminal events/own executions. [PG2], [PG6], [MY3], [MS3], [MS5]
- **Identity/coverage:** use engine key plus database/user/node and generation (PG query ID; MySQL digest/schema; SQL Server plan handle/statement offsets/generation before optional hash grouping; ClickHouse normalised hash with database/node). Never rely on a hash globally or merge differently scoped memory. Bounded event histories use event identity + server generation and overlap/deduplication; skipped samples, evicted digests, cache entries, dropped logs and row caps are visible coverage gaps. Query Store interval totals are upserted/reconciled, not repeatedly added as new events. [PG2], [MY3], [MS3], [MS5], [CH2]
- **Detail / Explain:** an explicit “Explain” action reuses the existing visualiser for supported engines, with plan-only EXPLAIN by default. Redacted/digest SQL is not executable: if no complete, user-reviewed statement/bindings/context is available, the click opens an editor to supply them; never guess parameter values or revive hidden literals. Use a separate explicit diagnostic session with the selected database/schema and current safety policy, not the monitor or a user's open transaction. Show changed context/plan age. ConfirmAll/AuthenticateAll and lock checks still apply to plan-only actions.
- **Diagnostic privileges:** access to statistics does not grant access to explain the referenced tables. Apply the statement's object privileges; MySQL additionally requires `SHOW VIEW` for explained views, SQL Server `SHOWPLAN` in every database containing referenced objects. Missing privileges produce a detail-level action state, while monitoring continues; never upgrade the monitor login. [PG8], [MY12], [MS15]
- **Measured execution:** ANALYZE/actual plans are separate, always explicit and confirmed, including SELECT. Recheck immutable statement, parameters, connection/context and safety policy in core (#99); OS authentication where required, cancellation/deadline, production workload warning. ReadOnly blocks possible writes; engine refusal of unsafe writes remains. Rollback cannot undo all effects, so never describe it as harmless. No automatic ANALYZE to obtain advisor evidence.

Resource panels show sessions by state, long/idle transactions, wait categories and blocking chains, cache efficiency, I/O rates, memory by documented scope and replication status. Unsupported per-query memory stays unavailable; never estimate it from query latency or app RSS. Aggregate completed waits are not current waits; cross-source snapshots are approximate. Self-monitoring queries are excluded by session/query identity where possible and otherwise labelled. No per-query CPU attribution is derived from server CPU gauges.

### Optimisation advisor

Rules are deterministic, versioned and engine/capability-gated. Each card includes rule ID/version, exact metric or plan node, observation window/coverage, threshold, object/query identity, confidence, expected qualitative impact and trade-offs. Sort by evidence-backed impact then stable ID; thresholds are inspectable. Missing data disables a rule rather than inventing evidence. Accepted initial rule defaults below are validation targets, not universal tuning advice.

| Rule | Trigger / evidence | Proposal and confidence boundary |
| --- | --- | --- |
| Large selective scan / missing index | Plan full/sequential scan on ≥ 100,000 estimated rows, ≤ 1% estimated predicate selectivity and no suitable index; or SQL Server missing-index DMV evidence. | Inspect predicate/index candidate and compare a plan. Medium confidence from estimates, higher with explicitly obtained actuals; a full scan alone is not a fault. Account for write/storage cost and existing index prefixes. DMV estimated impact is not a promised speed-up. |
| Unused / duplicate index | Zero observed index reads over a known representative period, or matching ordered keys/include columns/predicate/uniqueness/collation metadata; PG index stats / MySQL/MariaDB `sys` evidence where available. | Review, never auto-drop; a 10 min window cannot establish unused. Exclude primary/unique/constraint-supporting indexes from drop proposals; flag seasonal work, replicas and reset gaps. Low confidence for inactivity, medium for equivalent definitions. |
| Estimate error | Existing measured plan shows ≥ 10× estimated/actual row mismatch (with zero handled explicitly). | Investigate statistics, correlated predicates or skew; explain node and actuals age. Suggest reviewed statistics maintenance where applicable, never execute it. Medium confidence; unavailable without actuals. |
| Sort/hash spill | Non-zero temp/spill counter delta or an actual-plan spill node; scope and units retained. | Review cardinality, projection, index/order and only then memory settings. High confidence spill occurred; impact/workload trade-offs remain uncertain. No automatic global memory change. |
| Possible N+1 pattern | Same digest ≥ 1,000 calls/min, mean ≤ 10 ms, stable across ≥ 3 complete intervals. | Investigate batching/caching in the application. Low confidence: statistics cannot prove request-level N+1; never claim the application call path is known. |
| Lock hot spot | Same blocking object/session seen in ≥ 3 samples or ≥ 1 s wait, with bounded chain and transaction age. | Inspect long transactions/access order and application scope. Medium confidence; snapshots can miss transient edges. Cancel/kill stays an explicit existing action with safety gates. |
| Vacuum / bloat indicator | PG ≥ 100,000 estimated dead tuples, dead/(live+dead) ≥ 20%, last vacuum/autovacuum ≥ 24 h ago and ≥ 3 samples; unknown maintenance age is insufficient evidence. | Inspect autovacuum settings, long transactions and maintenance schedule. Low confidence bloat hint, not a byte/% measurement; vacuum/fillfactor changes only as reviewed proposals. |

Generated DDL or rewrites open an **editor tab** with evidence and warnings, never auto-run; identifiers are quoted and unsupported rewrites are prose, not fabricated SQL. Execution goes through current safety/authentication and app-lock gates. Production profiles show an extra impact warning (locks, I/O, storage, write cost); do not waive ReadOnly or authentication because a card was accepted.

Optional HypoPG validation is off, only when already installed and explicitly enabled: probe access, use an isolated diagnostic session, bounded `hypopg_create_index`, plan-only EXPLAIN, and `hypopg_reset`/session close cleanup. It changes hypothetical session state, so it is not a polling source and must not use the monitor session; hypothetical indexes never establish runtime improvement. No automatic extension install. [HP1]

[docs/23's assistant](../23-ai-assistant.md#4-data-sharing-policy-per-connection) integrates through the **M4.5 database harness** ([docs/28 §7](../28-database-monitoring-and-ai-harness.md#7-database-investigation-harness)), independently of monitoring and deterministic rules. M2.5 sends nothing to providers/MCP. M45-10..12 require separately reviewed telemetry permission, payload preview/redaction, allowlists and bounded read/propose tools under [ADR-0009](0009-native-ai-byok-cli-bridges-mcp.md); sampled literals and credentials remain excluded. No AI write/execute path is added.

### UX, privacy and permission states

Use [docs/20](../20-ux-foundations.md)'s neutral greys, compact rows, tokens, subtle separators and visible environment context. Connection menu/command palette → **Monitor**; one tab per workspace. Toolbar: Pause/Resume, interval, time window (1/5/10 min), selected database/node, “last updated” and effective adaptive cadence. Overview tiles lead to **Live sessions** (reuse process-list view), **Top queries**, **Waits & locks**, **Resources** and **Advisor**. Narrow layouts stack tiles and use the existing trailing inspector; Boards, History and Reports use the same connection workspace and evidence models defined in docs/28; no competing global navigation model.

| State | Required presentation / action |
| --- | --- |
| Ready / partial | Units, scope, sample age and coverage; caps/dropped events are explicit. Chart/table data comes from the same samples. |
| Permission required / restricted | Source name and minimum grant/action, own-user/database scope if that is all the server permits; request DBA/provider help and offer copyable reviewed grant instructions. Do not infer full visibility from a successful empty SELECT. |
| Disabled / setup required | Extension/consumer/Query Store requirement, overhead/restart warning and “Show setup instructions”; healthy sources still work. |
| Unsupported / own-session | Exact engine/version/transport reason beside unavailable panels; embedded/remote-libSQL wording above remains visible, not hidden in a tooltip. |
| Stale / paused / failed | Retain last permitted sample with age, visible gap and error kind; suppress stale advice, manual re-probe/reconnect. Disconnect/close clears memory; app lock stops sampling and hides/clears sensitive payloads before unlock. No hidden collection while paused. |

All controls/tables have accessible names and keyboard operation; stable row selection and focus through refresh, no disruptive auto-sort while inspecting, textual chart summaries/units and non-colour-only states. Announce source transitions sparingly, not every tick. Respect reduced motion/font scaling; verify contrast, VoiceOver/NVDA and keyboard-only flows against NFR-UX-01..03.

Raw source text can contain credentials, personal data, comments, utility literals and plan constants even when called “normalised”. In core, remove comments, replace every string/numeric/binary/date literal and parameter value, apply connection redaction rules, strip plan constants/SQL fragments and sensitive identifiers where configured. If dialect parsing/redaction is incomplete (truncated/unknown syntax), expose fingerprint/metrics only. Redact before Arrow/IPC, rings, history, clipboard/export or advisor evidence. Own executions follow the same rule. No automatic raw-text reveal; an editor statement supplied for Explain follows existing explicit editor behaviour. Logs contain kinds/counts/durations only, never SQL, literals, raw plans or server error messages, with [docs/12](../12-security.md) and [docs/14 §6](../14-reliability-and-observability.md#6-logging) scrubbing as defence in depth. Credentials and sampled literal/parameter values never leave the machine via monitoring telemetry, advice or exports; normal authenticated database transport remains necessary. M4.5 may share authorised redacted aggregate evidence only through its separately reviewed sharing boundary. Board definitions and user-requested result exports follow docs/28's explicit preview and existing data-sharing policy; they do not weaken collector redaction.

## Consequences

- Operators can see work from other applications where engine permissions/statistics allow it, with explanations for missing data and concrete tuning evidence.
- Dedicated server sessions preserve query/transaction state; shared collectors, bounded samples and source deadlines bound overhead. Extra connections consume server slots and app memory; existing budgets remain release gates.
- Engine features, refresh lag, cumulative resets and privacy differ. Capability/coverage states are part of correctness, not optional polish. No guarantee of all short queries, per-query RAM on every engine or exact bloat is made.
- Embedded own-session capture must integrate with host/actor lifecycle and cannot provide a server-wide substitute. Query-backed boards, alerts and reports are planned in the same M2.5 product scope (docs/28; docs/26 A08–A12/G07), reusing these bounds and privacy rules. External exporters/collectors, NoSQL monitoring and automatic tuning remain separately scoped.

## Alternatives considered

| Option | Disposition |
| --- | --- |
| Agent-based collectors | Better OS metrics/lossless tracing, but adds server installation, elevated access, deployment/trust and compatibility work. Defer to a separately reviewed integration; no agent prerequisite. |
| Prometheus exporters | Useful existing fleet history/alerts; needs endpoints, credentials and source-to-connection mapping. Optional later adapter, not a dependency or silent external data route. |
| Persistent time-series database | Supports long retention but adds service/storage/migration burden and disk writes by default. Reject for baseline; bounded opt-in local aggregate history suffices. |
| AI-only advice | Non-deterministic, hard to validate and may transmit sensitive workload data. Reject as baseline; deterministic evidence-first rules, optional later docs/23 layer. |
| Process polling / own-query timing only | Reuse as one input, but misses short external queries and cumulative costs. Insufficient for server engines; honest own-session scope for embedded engines. |

## Testing and implementation plan

The architecture is accepted; the M25-01/02 backend slice is implemented with validation gaps listed below. The matrix below supplies detailed source/safety gates for [docs/27 M2.5](../27-delivery-plan.md#5a-m25--database-monitoring-and-analytics), which alone owns task status/dependencies. [docs/28 §8](../28-database-monitoring-and-ai-harness.md#8-acceptance-and-verification) adds product/board/report and M4.5 harness acceptance. The matrix remains the full validation contract; the implementation evidence below distinguishes executed tests from pending gates.

| Step | Scope | Done when / evidence required |
| --- | --- | --- |
| M25-01/03 | Monitoring session, capabilities, scheduler, Arrow sampling and optional history | Independent server session never touches user transaction/startup SQL; shared subscriber lifecycle, app lock, pause, slow/error back-off, generation races and cancellation verified. Virtual-time tests cover deltas, zero denominators, resets/rollover, source maxima, caps/eviction and expiry/purge. Privacy fixtures with utility text/comments/plan constants fail closed before IPC/logs/history. Synthetic 1,000-row benchmark meets analysis/memory caps; licence review for any new crate. |
| M25-02 | PostgreSQL sources/setup guidance | PG 12 legacy and 13+ counter layouts, 16+ I/O, restricted/denied/full stats roles, extension missing/unloaded/disabled and managed-provider denial verified. External workload, lock chain, temp spill, vacuum indicators and reset gaps appear with scope; no enabling/reset/maintenance side effect. Measure collection overhead against the same workload without monitoring. |
| M25-07a | MySQL/MariaDB sources | MySQL 5.7/8.0/8.4 and MariaDB 10.6/11.4 matrix covers disabled instrumentation, restricted grants, history overwrite, picosecond conversion, different lock/sys layouts, memory unsupported vs 8.0.31+, replication grants and reset/truncation. External workload visible without instrumentation changes; deadlines and partial coverage evidenced. |
| M25-07b | SQL Server sources | SQL Server 2017/2019 and 2022+ privilege split plus Azure SQL scoped/denied fixtures; Query Store off/read-only/active-interval reconciliation, cached-plan eviction/recompile, grants/spill units and missing-index limits verified. No query replay/Query Store enabling; actual/estimated-plan safety flows and independent-session lifecycle checked. |
| M25-07c | ClickHouse sources | 25.3 LTS fixtures: other-user visibility, query-log disabled/sampled/delayed, terminal-event deduplication, node vs initial/shard scope, memory tracking gaps and async gauges. Time/read/result caps prevent historical scans; no flush/config mutation; missing replica/source states explicit. |
| M25-03/04 | Monitor tab and safe Explain integration | Overview/live/top/waits/resources/advisor, pause/interval/window, every capability state and own-session notices verified. Existing visualiser reused; redacted SQL opens review when incomplete; every actual execution confirms and passes core safety/auth/lock gates. Keyboard/axe, VoiceOver/NVDA, both themes/narrow layout and focus across refresh evidenced. Reference-machine CPU/footprint and unchanged docs/13 budgets pass at M2.5 exit. |
| M25-10 | Deterministic advisor and optional HypoPG | Positive/negative/insufficient/stale-evidence fixtures for every rule above, stable results/thresholds/confidence and evidence pointers. No automatic SQL; proposal editor/production/ReadOnly/auth gates verified. HypoPG absent/disabled/denied, isolated validation and cleanup tested without installation; no provider calls. |
| M25-07d/07e | Embedded / remote-libSQL own-session capture | SQLite 3.20+ counter semantics/compile-option denial; libSQL file follows SQLite, remote Hrana exposes only own requests. DuckDB 1.4 LTS+ function/profile probes, host-loss and no-output profile capture verified; writable-file actor never gains a second connection, periodic jobs do not delay user work, busy stats labelled unavailable. Other-process workloads are never claimed visible. |

At implementation, disposable fixtures generate external traffic alongside OpenRows; no production inspection. Record collection duration/query count, server CPU and throughput/latency change (target ≤ 1% throughput/latency overhead under the reference workload), app CPU/all-process footprint and 1,000-row analysis p95. Check default and minimum intervals, network latency, many digests/blocked sessions and global collector caps; reject or throttle expensive sources, do not solve misses by silently increasing budgets. Benchmark on docs/13 reference hardware; cleanup covers monitor/diagnostic sessions, hosts, history fixtures and containers.

## Owner decisions — 2026-10-11

The owner accepted all ADR-0011 recommended defaults, with the docs/28 placement and expanded scope taking precedence. These decisions bind M2.5; the database AI integration is separately reviewed in M4.5. Validation remains pending.

1. **Placement and scope — accepted.** Monitoring plus analytics is M2.5 immediately after M2's DuckDB host repair; FR-ADMIN-03..08 are P1. The database AI harness is M4.5 (FR-AI-11..13). ADR-0011 is the architecture decision and docs/28 the product/feature specification.
2. **Polling and live window — accepted.** 5 s live polling, ≥ 15 s digest/resource and ≥ 60 s expensive sources, adaptive back-off capped at 60 s, 10 min live window; the table above defines source, row, memory and global limits. No background collection by default.
3. **Local history — accepted.** Off by default; opt-in redacted aggregates, 24 h / 50 MiB per connection / 100 MiB app cap, expiry and explicit purge.
4. **Instrumentation setup — accepted.** Guidance plus editable reviewed SQL only; never automatic enabling, elevated login, restart, grants or global changes.
5. **Explain and measured execution — accepted.** Plan-only default; every measured execution explicitly confirms and passes #99 safety/authentication/app-lock gates, with a production warning. No silent rollback-based execution.
6. **Hypothetical and AI advice — accepted.** HypoPG off by default, opt-in only if already installed, isolated diagnostics and cleanup. AI integration uses the M4.5 harness with separately reviewed policy/context and tool boundaries.
7. **Server and app budgets — accepted.** Adopt all stated budgets as initial gates, including the ≤ 1% reference-workload server overhead target and docs/13 aggregate limits. Throttle a source on a miss or revise this ADR explicitly; no silent budget increase.

## Validation / exit criteria

The owner accepted the architecture on 2026-10-11. M25-09 validates the source/version/privilege matrix, evidence-backed advisor, safe Explain flows, privacy/lock/retention tests, accessible native-app journeys and reference-hardware overhead numbers, together with docs/28's analytics/alert/report gates. M45-09 validates the optional AI harness independently. A second writable DuckDB connection, default persistence, unbounded sampling, raw sampled values crossing IPC/logs or automatic execution invalidates the design. Revisit cadence/source choice after profiling an overhead miss; revise scope explicitly if continuous tracing, host profiling or fleet retention becomes necessary.

## Initial design validation / evidence — 2026-10-11

Documentation only: the owner decisions and merged M2.5/M4.5 scope are recorded consistently in requirements, roadmap, parity, delivery tasks and ADR index. Round-1 source pointers and official vendor references below are retained. No implementation, application journey, database fixture or performance result is claimed; M25-09 and M45-09 validation remains pending.


## Implemented (M25-03): workspace/history evidence

Final M25-03 quality evidence: workspace Rust `-E 'not binary(ssh_container)'`
passed 1,455 tests (198 skipped); Vitest passed 2,766 tests across 243 files.
Formatting, Clippy/ESLint, TypeScript, licence/dependency scans, hygiene, UI
tokens/colours, bundle budgets and IPC freshness all passed.


[docs/28 §10](../28-database-monitoring-and-ai-harness.md#10-implemented-m25-03-monitor-workspace-and-local-history)
records the PostgreSQL UI, default-off scalar history and development checks.
The lazy tab transfers bounded Arrow pages to a disposable worker, serialises
refreshes and rejects stale generations; source changes alone trigger reads.
SVG charts preserve missing states and provide keyboard-readable table views.
Existing Processes and plan visualisation supply guarded actions; collected SQL
opens explicit review before plan-only Explain. No measured execution action
or automatic setup SQL is added.

LocalStore migration 0007 allowlists scalar names and finite values only.
Profile/database keys isolate history; configuration atomically purges disabled
profiles. Startup/read/write expiry enforces 24 hours; bounded keys and a
conservative 2-KiB row charge enforce 50/100 MiB retained-data limits. Per-profile
and global purge retain the explicit opt-in choice. At most 300 history buckets
per metric keep gaps ahead of valid values in the same bucket. Shared SQLite
free pages may remain after purge; these caps describe retained monitor data,
not the complete application store file. Connection opt-ins are excluded from
portable preferences and enabling never opens a collector.

On Apple M4/24 GiB, native arm64 PostgreSQL 17.11 integration passed 79 tests.
Source read plus guards p95: 9.824 ms; 1,000-row analysis p95: 4.361 ms;
60-second process CPU: 0.09 s; RSS before/after: 17,856/16,768 KiB; retained live
sample estimate: 332,773 bytes. Actual redacted Arrow samples displayed a
blocker/waiter chain and positive throughput in the UI components.
Eleven ten-minute charts (300 timestamps each) in Chromium measured render
p95/max 8.3/9.3 ms and frame p95/max 16.7/16.8 ms, with no >50 ms dev-monitor
reports. The initial production JavaScript is 305,167 gzip bytes against the
350,000-byte budget. Native WKWebView, mac-low all-process footprint, matched-load server
impact and the outstanding engine/source matrix remain M25-09 validation.


## Implemented (M25-01/02): evidence

The PostgreSQL backend and typed IPC described in [docs/28 §9](../28-database-monitoring-and-ai-harness.md#9-implemented-m25-0102-monitoring-runtime-and-postgresql-backend)
implement this ADR's collection/privacy boundary. No UI/history/other-engine
support is implied. Core tests cover null unknown/gap/reset cells, exact integer
deltas/rollover and zero-denominator handling, adaptive minima, series/time/row
caps, pagination, redaction goldens, serial reads, cancellation and terminal
session discard/read-time expiry. Desktop tests cover shared admission and
close/lock invalidation of pending generations, replacement-safe cancelled-open
cleanup, UUID aliases, redacted Processes reuse and
control/binary IPC contracts. Generated Specta bindings include the commands,
window request and status event; the binary commands return bounded Arrow.

Native-arm64 container evidence: `just test-pg 17` and `just test-pg 14`, each
79 tests passed. The monitor fixture exercises active work, a blocked writer,
bounded chain output, positive xact rates over two samples, installed/missing
statement extension, a restricted login, reviewed grant and subsequent
revocation, relations/indexes, a real database-statistics reset from a separate
administrative fixture session, and no-replica/disabled-track states. PG12's
monitor fixture passed with the legacy statement layout; the full PG12 suite
fails in the existing `types::times_networks_and_ranges_decode_natively` test
because `datemultirange` is unavailable. An intermittent PG14 cancellation
conformance failure passed on rerun without modifying that test.

The opt-in `OPENROWS_BENCH=1 just test-pg 17` reports 60-second collector-process
CPU/RSS, default-cadence data-query counts, client durations including guards,
server statement durations and synthetic 1,000-row analysis p95. This is an
Apple M4 / 24 GiB / macOS 26.5 development-machine run, not the mac-low reference
or an all-app physical-footprint checkpoint. Final measurements:

| Measurement | Development-machine result | Budget / limitation |
| --- | --- | --- |
| Data reads over 60 s | 27: activity 12, database/statements/I/O 4 each, tables/indexes/replication 1 each | Default source floors; discovery excluded; changing-source schedule |
| Client read plus guards p95 | 30.652 ms | Default deadline 1,000 ms; includes round trips |
| Covered server statements | 647 calls, 61.090 ms total, 9.097 ms maximum | Includes fixture/guard commands; no matched-load baseline |
| Collector process CPU | 0.08 s over 60 s, about 0.13% of one core | Component evidence only; ≤1% all-app mac-low budget uncertified |
| Collector process RSS before / after | 17,184 / 16,000 KiB | RSS snapshot, not all-app physical footprint |
| Retained live sample estimate | 305,322 bytes | Implementation cap 2 MiB/monitor |
| 1,000-row analysis p95 | 3.407 ms | ≤10 ms; 100 owned-sample ingestions |
| 1,000-row retained estimate | 1,363,086 bytes | Implementation cap 2 MiB/monitor |

The benchmark does not compare identical loaded
server throughput/latency with and without monitoring; the accepted ≤1% server
impact and ≤32 MiB all-process monitor increment remain uncertified. No budget
is increased. Current 2 MiB retained/monitor admission is stricter than the
16 MiB ceiling; four collectors and serialized Arrow encoding bound retained
and in-flight estimates but do not replace physical-footprint measurement.

Final serial workspace evidence: 1,389 tests passed, 198 skipped (197 ordinary
ignored integration tests plus the explicitly excluded two-hop SSH test). The
existing two-hop SSH fixture requires two containers and was not run under
this task's one-container rule, so the full gate remains incomplete. Initial
shared-cache fixture-path failures and host cancellation failures passed on
rerun, including both `conformance::cancel_contract` and
`duckdb_conformance::cancel_contract` in the final workspace run. No deadlines
or tests were weakened. Rust incremental compilation was disabled within the
same shared target after stale fixture paths referenced another worktree;
`sccache` and the shared build cache remained in use. The focused monitoring
suite (15), frontend tests (2,713), workspace Clippy and binding freshness passed.

Pending matrix: PG13/15/16/18 live suites, managed-provider denied probes,
installed-but-unloaded extension, replication lag, temp spill, vacuum and
statement/I/O statistics-reset fixtures, minimum-cadence/latency/many-digest workload variants,
mac-low aggregate CPU/footprint and matched reference server load. The PG18 I/O
adapter uses `read_bytes`/`write_bytes` after a column/layout probe; PG16/17 use
`reads`/`writes × op_bytes`. Older statement reset continuity remains labelled
best effort. Per-query CPU/memory and latency percentiles are unavailable.
Statement rankings cover the latest valid sample interval, with full-window
aggregation deferred to M25-04. Startup SQL is never replayed by the monitor;
pause/lock requires explicit restart, and profile edits invalidate collection
until the user reconnects. All samples are in memory only.

## Implemented (M25-07a): MySQL/MariaDB evidence

Separate probed adapters implement MySQL 5.7/8.0/8.4/9.x and MariaDB 10.6–11.x
layouts; live acceptance fixtures ran on native-arm64 MySQL 8.4 and MariaDB 11.4.
Read-only transactions apply `max_execution_time` on MySQL and
`max_statement_time` on MariaDB before source reads. Client deadlines supplement
SELECT limits, including bounded SHOW status commands. Failed guards disable
reads; partial responses invalidate the dedicated transport. At most fourteen
discovery reads and one data query/due source reuse the existing runtime floors
(5/15/60 s), adaptation, memory admission and core redaction. Blocking SQL caps
InnoDB edges; the core bounds graph depth and flags unknown owners/cycles.

Digest timers divide picoseconds by 1e9 once. Identity includes schema, digest
and first-seen time; uptime decreases, status flush markers and decreasing
counters reset rate baselines. Reset/truncation not exposed by the engine remains
best effort. Disabled Performance Schema/timing/consumers, PROCESS, SELECT and
replication grants have actionable states. MariaDB SLAVE MONITOR/REPLICA MONITOR
alias guidance is separate from MySQL REPLICATION CLIENT. Instrumentation setup
in disposable acceptance fixtures is administrative test code only.

Two engine facts found by the live fixtures shape the collector. Without
PROCESS, both engines return only the caller's own threads instead of an error,
so discovery probes `INNODB_TRX` and reports activity as permission-denied
naming PROCESS. MySQL has no deadlock status variable (MariaDB's
`Innodb_deadlocks`); when the enabled `INNODB_METRICS.lock_deadlocks` counter is
readable (PROCESS, Performance Schema on) the MySQL status read selects the
whitelisted `global_status` rows plus that metric, otherwise `SHOW GLOBAL STATUS`
is used and deadlocks stay unknown with a coverage note. InnoDB also caches
`INNODB_TRX`/lock waits and refreshes only after 100 ms without readers; the
default 5 s cadence is unaffected and the fixture polls at 250 ms. `sys` index
views are probed on both engines (MariaDB ships `sys` from 10.6).

`just test-mysql` (MySQL 8.4 then MariaDB 11.4, one container at a time,
`--performance-schema=ON`) passed all 48 tests per image, including
`monitor::activity_blockers_rates_digest_and_restricted_grants`: an active
waiter query visible, a `SELECT … FOR UPDATE` blocker/waiter chain, query and
deadlock rates over two samples, digest top-N with redacted literals, no-replica
state, and a restricted login whose PROCESS/replication denials name the grant,
followed by a granted PROCESS and a revoked Performance Schema SELECT.
`OPENROWS_TEST_MYSQL_PERFORMANCE_SCHEMA=OFF just test-mysql mariadb:11.4` passed
all 48 tests: statement/table/index/I/O sources report "performance_schema
disabled … restart" while activity, InnoDB blocking and counters still work.
The same monitor fixture also passed on MySQL 8.4 started with Performance
Schema off (blocking is then unavailable on MySQL and skipped). Unit tests cover
the version matrix, instrument/consumer gating, grant/deadline error mapping,
bounded rows, replica field filtering and uptime/flush resets.

The opt-in `OPENROWS_BENCH=1 just test-mysql` reports the same measurements as
the PostgreSQL benchmark. Apple M4 / 24 GiB / macOS 26.5 development machine,
not mac-low or an all-app footprint checkpoint:

| Measurement | MySQL 8.4 | MariaDB 11.4 | Budget / limitation |
| --- | --- | --- | --- |
| Data reads over 60 s | 27: activity 12, database/statements/I/O 4 each, tables/indexes/replication 1 each | 27, same split | Default source floors; four round trips each with guards; discovery excluded |
| Client read plus guards p95 | 11.335 ms | 10.770 ms | Default deadline 1,000 ms; includes round trips |
| Collector process CPU over 60 s | 0.18 s, about 0.30% of one core | 0.19 s, about 0.32% of one core | Component evidence only; ≤1% all-app mac-low budget uncertified |
| Collector process RSS before / after | 20,464 / 20,992 KiB | 20,272 / 20,704 KiB | RSS snapshot, not all-app physical footprint |
| Retained live sample estimate | 554,166 bytes | 524,245 bytes | Implementation cap 2 MiB/monitor |
| 1,000-row analysis p95 | 2.464 ms | 3.016 ms | ≤10 ms; 100 owned-sample ingestions |

Server-side digest totals in the benchmark include fixture and guard statements
and are not a matched-load comparison; the ≤1% server impact and ≤32 MiB
all-process increment remain uncertified. No budget is increased.

Wider live MySQL 5.7/8.0/9.x and MariaDB 10.6/11.x/provider, enabled replication
topology, history overwrite, reference workload throughput/latency and mac-low
all-app CPU/physical footprint remain M25-09 verification.

## Implemented (M25-07b): SQL Server evidence

The collector accepts SQL Server 2017+, Managed Instance, Edge and Azure SQL
Database (database-scoped DMVs and `sys.dm_db_wait_stats`). Each read is one
batch with `SET LOCK_TIMEOUT 100`; client cancellation sends the TDS attention
signal at three quarters of the source deadline, and a missing acknowledgement
makes the reading terminal. Missing VIEW SERVER STATE (VIEW SERVER PERFORMANCE
STATE on 2022+, VIEW DATABASE STATE on Azure SQL Database) is a denied state for
every DMV source, because SQL Server otherwise returns only the caller's session.
Top queries group `sys.dm_exec_query_stats` by `query_hash` (optional hash
grouping) and filter on the `dbid` plan attribute, because ad hoc text has no
`dbid`. Performance counters are prefiltered by name: an unfiltered pivot of
`sys.dm_os_performance_counters` cost about 320 ms per read on Azure SQL Edge.
Its `object_name` is padded `nchar`, so suffix matching trims it first. Query
Store, Always On health and live SQL Server 2022/Azure fixtures remain M25-09.

## Vendor references

Official references consulted 2026-10-11; version-specific adapters must recheck their server's documentation at implementation.

[PG1]: https://www.postgresql.org/docs/12/monitoring-stats.html
[PG2]: https://www.postgresql.org/docs/12/pgstatstatements.html
[PG3]: https://www.postgresql.org/docs/13/pgstatstatements.html
[PG4]: https://www.postgresql.org/docs/12/functions-info.html
[PG5]: https://www.postgresql.org/docs/16/monitoring-stats.html
[PG6]: https://www.postgresql.org/docs/current/pgstatstatements.html
[PG7]: https://www.postgresql.org/docs/12/runtime-config-client.html
[PG8]: https://www.postgresql.org/docs/12/sql-explain.html
[MY1]: https://dev.mysql.com/doc/mysql-perfschema-excerpt/5.7/en/performance-schema-table-reference.html
[MY2]: https://dev.mysql.com/doc/mysql-perfschema-excerpt/5.7/en/performance-schema-statement-tables.html
[MY3]: https://dev.mysql.com/doc/refman/8.0/en/performance-schema-statement-summary-tables.html
[MY4]: https://dev.mysql.com/doc/refman/8.0/en/sys-schema.html
[MY5]: https://dev.mysql.com/doc/relnotes/mysql/8.0/en/news-8-0-31.html
[MY6]: https://dev.mysql.com/doc/refman/8.0/en/innodb-information-schema-metrics-table.html
[MY7]: https://dev.mysql.com/doc/refman/8.0/en/privileges-provided.html
[MY8]: https://dev.mysql.com/doc/mysql-perfschema-excerpt/8.0/en/performance-schema-data-lock-waits-table.html
[MY9]: https://dev.mysql.com/doc/refman/8.0/en/show-replica-status.html
[MY10]: https://dev.mysql.com/doc/refman/8.0/en/optimizer-hints.html
[MY11]: https://dev.mysql.com/doc/refman/8.0/en/performance-schema-table-characteristics.html
[MY12]: https://dev.mysql.com/doc/refman/8.0/en/explain.html
[MY13]: https://dev.mysql.com/doc/refman/8.0/en/sys-schema-prerequisites.html
[MA1]: https://mariadb.com/docs/server/reference/system-tables/performance-schema/performance-schema-overview
[MA2]: https://mariadb.com/docs/server/reference/system-tables/performance-schema/performance-schema-tables/performance-schema-events_statements_summary_by_digest-table
[MA3]: https://mariadb.com/docs/server/reference/system-tables/sys-schema
[MA4]: https://mariadb.com/kb/en/schema_object_overview-sys-schema-view/
[MA5]: https://mariadb.com/docs/server/reference/system-tables/performance-schema/performance-schema-tables/performance-schema-memory_summary_global_by_event_name-table
[MA6]: https://mariadb.com/docs/server/reference/system-tables/information-schema/information-schema-tables/information-schema-innodb-tables/information-schema-innodb_metrics-table
[MA7]: https://mariadb.com/docs/server/reference/system-tables/information-schema/information-schema-tables/information-schema-innodb-tables/information-schema-innodb_lock_waits-table
[MA8]: https://mariadb.com/docs/server/reference/sql-statements/administrative-sql-statements/show/show-replica-status
[MA9]: https://mariadb.com/docs/server/ha-and-performance/optimization-and-tuning/query-optimizations/aborting-statements
[MS1]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-requests-transact-sql
[MS2]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-sessions-transact-sql
[MS3]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-query-stats-transact-sql
[MS4]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-query-plan-transact-sql
[MS5]: https://learn.microsoft.com/en-us/sql/relational-databases/system-catalog-views/sys-query-store-runtime-stats-transact-sql
[MS6]: https://learn.microsoft.com/en-us/sql/relational-databases/performance/monitoring-performance-by-using-the-query-store
[MS7]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-os-wait-stats-transact-sql
[MS8]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-os-memory-clerks-transact-sql
[MS9]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-exec-query-memory-grants-transact-sql
[MS10]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-io-virtual-file-stats-transact-sql
[MS11]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-os-performance-counters-transact-sql
[MS12]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-db-missing-index-details-transact-sql
[MS13]: https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-objects/sys-dm-hadr-database-replica-states-transact-sql
[MS14]: https://learn.microsoft.com/en-us/sql/t-sql/statements/set-lock-timeout-transact-sql
[MS15]: https://learn.microsoft.com/en-us/sql/t-sql/statements/set-showplan-xml-transact-sql
[CH1]: https://clickhouse.com/docs/reference/system-tables/processes
[CH2]: https://clickhouse.com/docs/reference/system-tables/query_log
[CH3]: https://clickhouse.com/docs/reference/system-tables/metrics
[CH4]: https://clickhouse.com/docs/reference/system-tables/events
[CH5]: https://clickhouse.com/docs/reference/system-tables/asynchronous_metrics
[CH6]: https://clickhouse.com/docs/reference/system-tables/replicas
[CH7]: https://clickhouse.com/docs/reference/statements/grant
[CH8]: https://clickhouse.com/docs/concepts/features/configuration/settings/query-complexity
[CH9]: https://clickhouse.com/docs/concepts/features/configuration/settings/permissions-for-queries
[SQ1]: https://www.sqlite.org/c3ref/stmt_status.html
[SQ2]: https://www.sqlite.org/c3ref/db_status.html
[SQ3]: https://www.sqlite.org/c3ref/stmt_scanstatus.html
[SQ4]: https://www.sqlite.org/releaselog/3_20_0.html
[DU1]: https://duckdb.org/docs/current/sql/meta/duckdb_table_functions
[DU2]: https://duckdb.org/docs/current/configuration/pragmas
[DU3]: https://duckdb.org/docs/current/dev/profiling
[DU4]: https://duckdb.org/docs/current/connect/concurrency
[LI1]: https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_3_SPEC.md
[HP1]: https://hypopg.readthedocs.io/en/rel1_stable/usage.html

## Implemented (M25-04/10): investigation/advisor evidence

The PostgreSQL implementation and exact rule/regression thresholds are documented
in [docs/28 §11](../28-database-monitoring-and-ai-harness.md#11-implemented-m25-0410-postgresql-investigation-and-advisor).
Core tests cover every rule with positive, negative, missing and stale evidence,
weighted interval means, reset/gap/identity churn, bounded retention, conservative
predicate projection and exact quoted proposal SQL. Native/UI tests cover reviewed
plan-only requests, measured SELECT confirmation bound to SQL/context/policy,
ReadOnly refusal, privacy before IPC, proposal queue actions/dismissal/snooze and
blocker session links.

The current task's final acceptance report records the workspace gate results;
earlier partial-run counts are not treated as current verification.

The opt-in debug advisor benchmark ran on an Apple M4 Mac16,10, native arm64,
24 GiB RAM, using 1,000 catalog rows and 100 iterations per case. Distinct-index
analysis measured 2.787 ms median / 5.036 ms p95; duplicate-index analysis measured
1.330 ms median / 1.345 ms p95, both within the 10 ms p95 component budget. This
new path has no main baseline, and these measurements do not certify server
overhead or all-app CPU/RSS.

`just test-pg 17` ran against one task-owned PostgreSQL 17.11 container: 82
tests passed (one skipped), including the real selective-scan/index-removal,
spill, stale-statistics, unused/duplicate-index, blocking and negative
fixtures. The task container and TLS scratch directory were removed; unrelated
containers were left untouched.

Optional HypoPG remains off until explicit per-check opt-in and reviewed SQL.
The isolated session checks an already-installed extension, compares plan-only
estimated costs, resets hypothetical indexes and closes; absence, denial and
cleanup have native mock-session tests. No extension installation, server
setting changes, provider calls or automatic proposal execution are added.
A live installed-HypoPG fixture was unavailable. Reference-load/app
footprint/CPU and mac-low certification remain M25-09.
