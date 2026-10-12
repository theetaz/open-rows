# 28 — Database Monitoring, Analytics and the AI Harness

Status: M2.5 monitoring and analytics implemented for every engine in §4 (M25-01..08, M25-10; §9–§19); the M25-09 exit audit (§20, 2026-10-12) records what passed and what still needs reference hardware or the live engine matrix. The M4.5 harness is partly implemented (§6–§7). Scope expanded on 2026-10-11. Delivery order:
finish the current DuckDB host repair, build monitoring and analytics (M2.5),
then build the database AI harness (M4.5). This document owns the expanded
FR-ADMIN-03..08 and FR-AI-11..13 scope; [27](27-delivery-plan.md) owns task status. [ADR-0011](adr/0011-performance-monitor.md) is the accepted architecture decision (owner, 2026-10-11); its validation results are in §20 and the ADR. It owns detailed engine sources/grants, counter semantics, budgets, privacy and advisor rules; this document owns product scope and acceptance.

## 1. Product outcome and delivery boundary

A user can understand a database's current health, investigate what changed,
build charts from bounded queries, and export an evidence-backed report. The
monitor works without AI. With an explicitly configured provider, the assistant
can investigate the same evidence, propose queries and optimisations, and build
reports and charts through the same application tools.

The first vertical slice targets PostgreSQL. The shared contracts support all
implemented engines; subsequent slices add MySQL/MariaDB, SQL Server,
ClickHouse, SQLite/libSQL and DuckDB according to their actual capabilities.
An engine is not labelled supported until its collector and integration tests
pass. Existing create/query/cancel support alone is insufficient.

No OpenRows AI account or hosted monitoring service is required. Collection
runs on the user's machine while the app is open. Continuous collection with
the app closed requires a separately specified service; it is not implied by
scheduled in-app checks. No external notifications are sent automatically.

## 2. Monitoring workspace

The connection's **Monitor** tab contains these views:

| View            | Contents                                                                                                                                                                         |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overview        | Availability, round-trip latency, active/idle connections, throughput, errors, cache effectiveness, storage growth and replication where the engine exposes them.                |
| Queries         | Current activity, long-running queries, normalised statement rankings, call/time/row deltas, app-observed latency percentiles, regression comparisons and deterministic advisor proposals.                        |
| Locks and waits | Blocker/waiter graph, wait categories, oldest transactions, idle-in-transaction sessions, lock duration and deadlock counter deltas where available.                             |
| Resources       | Database/table/index sizes, engine buffer/memory/I/O counters, temporary files, spills, maintenance backlog and supported replication indicators.                                |
| History         | Time-range selection, previous-window comparison, restarts/statistics resets, gaps, annotations and alert transitions.                                                           |
| Boards          | Query-backed line, area, bar and scatter charts, scalar cards and tables; named, typed parameters and independently configurable refresh.                                        |
| Reports         | Current health summary, selected interval, findings, charts, affected objects/queries, evidence and recommended next steps; export Markdown, HTML, JSON, CSV and SVG/PNG charts. |

Every view shows source, collection time, scope and freshness. Unavailable,
permission-denied, not-applicable, stale and disconnected are distinct states;
none become zero or a green health indicator. Server-wide measurements are
labelled separately from current-database and OpenRows-session measurements.
CPU/RAM/host disk utilisation needs a separately configured collector when the
database does not expose it; desktop process usage is never presented as server
usage. The initial release does not install an OS agent.

Query rankings use normalised fingerprints by default. Collector text is redacted in core before IPC, rings, history, exports or advice; unsafe/truncated normalisation fails closed to fingerprints and metrics. There is no raw sampled-SQL reveal setting. A user supplies a complete reviewed statement/bindings in the editor for diagnostics; board definitions and explicit result exports retain their separate review/sharing boundary. Server summary
views that expose averages do not become invented p95/p99 measurements.
Percentiles need observed samples or a real histogram, with their population
and sample count shown. Index and rewrite recommendations remain proposals;
estimated plan cost is not a measured performance improvement. FR-ADMIN-05 includes the deterministic advisor: versioned rules, metric/plan-node evidence, coverage, confidence, impact and trade-offs, as defined in [ADR-0011](adr/0011-performance-monitor.md#optimisation-advisor). DDL/rewrites open an editor proposal only. Explain defaults to plan-only; incomplete SQL opens review, and every measured execution explicitly confirms under #99 safety/authentication/app-lock gates with a production warning. HypoPG is off unless already installed and explicitly enabled in an isolated diagnostic session; optional AI advice uses only the separately reviewed M4.5 harness.

## 3. Collection, storage and lifecycle

One core-owned collector per connection/database serves all views and windows.
Frontend timers do not open extra collectors. Metric descriptors specify unit,
gauge/counter/histogram kind, scope, source/version requirements and permission
requirements. Samples include wall time, monotonic interval, sequence, server
identity and statistics-reset identity. Delta calculations reject resets,
negative counters, changed identities and missing intervals.

Collection uses a bounded read-only administrative session and the existing
SSH/TLS/proxy routes, independently of a user's running query. DuckDB writable
files must retain their single owning host/session under ADR-0010: safe reads
queue through that owner (§19). Never create a hidden second writer or metadata
host to implement monitoring.

- Default live polling: 5 s; controls Off / 2 / 5 / 10 / 30 / 60 s. The selected
  cadence cannot override ≥ 15 s digest/resource or ≥ 60 s expensive-source
  minima. Adaptive back-off caps at 60 s; show effective cadence and freshness.
  The live window is 10 min. [ADR-0011](adr/0011-performance-monitor.md#architecture-sampling-and-overhead)
  owns the accepted source, row, series, collector and server/app budgets.
  Digest/resource means ≥ 15 s; expensive sources remain ≥ 60 s even when
  the live cadence is shorter.
- A slow sample never overlaps its successor. Cancellation interrupts the
  collector session; bounded query and lock timeouts prevent indefinite waits.
  Workload limits cover connections, time, rows, bytes and retained samples.
- Closing the last view stops collection unless the user enabled an explicit
  background monitor. Locking the app stops collectors and AI runs; unpausing
  rechecks connection state and permissions before admitting new work.
- Disconnect, profile deletion, shutdown and policy revocation cancel work and
  release sessions. Wake/reconnect marks the gap; historical interpolation does
  not invent observations. Hosted failures require explicit reconnect.
- An in-memory ring is bounded. Persisted history is opt-in, local, has a size
  cap and retention policy: redacted aggregates only, 24 h / 50 MiB per
  connection / 100 MiB app cap, expiry and explicit clear/history-disable purge.
  No statement text, result rows or credentials are persisted by the collector.
- Alert rules are deterministic and work without a model: threshold, required
  duration, severity, recovery threshold, cooldown and deduplication. An
  incident records onset/recovery and evidence. Unknown data cannot clear an
  incident or trigger a healthy notification.

Collectors never install extensions, enable global instrumentation, reset
statistics, grant roles, vacuum, create indexes or terminate sessions. Setup
guidance explains missing privileges/extensions and offers editable reviewed
SQL only. Users apply changes through the normal editor and safety policy,
never automatic enabling. Existing cancel/kill controls remain separate
explicit user actions.

### 3.1 Planned ownership

| Layer                                    | Responsibility                                                                                                        |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `openrows-driver-api` and engine drivers | Typed metric capabilities and bounded engine reads; no provider or UI dependency.                                     |
| `openrows-monitor` (new, Tauri-free)     | Shared collection, sample/rate/reset logic, history views, deterministic incidents and validated report/board models. |
| `openrows-store`                         | Opt-in local metric history, board definitions, retention and scrubbed audit metadata.                                |
| `openrows-ai` / `openrows-mcp`           | Provider adapters, policy/context, bounded investigation loop, read/propose tools and evidence-grounded artifacts.    |
| Desktop shell and typed IPC              | Connection/lock ownership, secret-store access, admission, cancellation and bounded commands/events/windows.          |
| Lazy React views and workers             | Monitor/boards/assistant presentation, chart mapping/downsampling, accessible tables and export previews.             |

The harness consumes the same monitor evidence and artifact validation as the
non-AI UI; it does not create a second collection or reporting implementation.

## 4. Engine coverage contract

Every row below has a collector with integration tests (§9–§19); §20 audits the
labels. [ADR-0011](adr/0011-performance-monitor.md#data-sources-and-permissions) owns
the detailed engine/version/source/privilege matrix and official vendor
references; do not duplicate or weaken it here:

| Engine             | Collector scope                                                                                                                | Limits to surface                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| PostgreSQL         | Activity, blocking/waits, database counters, relation/index sizes and maintenance, replication; optional statement statistics. | Version/role checks, extension availability, statistics resets and visibility restrictions.             |
| MySQL/MariaDB      | Process activity, status counters, buffer/storage and replication indicators; optional performance instrumentation.            | Separate engine/version adapters, instrumentation enablement, grants and managed-service restrictions.  |
| SQL Server         | Sessions/requests, waits/blocking, database/resource counters and optional query history.                                      | DMV permissions, edition/version and platform restrictions.                                             |
| ClickHouse         | Query activity/logs, parts/storage, engine counters and replication where applicable.                                          | Permissions, log configuration, cluster versus node scope and managed-service limits.                   |
| SQLite/libSQL file | App-observed queries/latency, file/page statistics and supported local engine diagnostics.                                     | No server process list or replication health; never call integrity checks on every tick.                |
| libSQL remote      | App-observed query/transport health plus explicitly exposed remote diagnostics.                                                | No fabricated remote OS/storage or server metrics.                                                      |
| DuckDB             | Own-session query health and engine diagnostics (database size, memory, temporary files, threads) through the owning session after query completion. | Single-writer reservation; a busy session leaves a gap; no server process list, waits or other processes. |

Each driver publishes a descriptor list with reasons for unsupported metrics.
Permission loss degrades affected collectors while preserving permitted data;
a partial overview stays visibly partial.

## 5. Analytics boards and report artifacts

A board is a versioned, secret-free JSON document with widget definitions,
connection references, SQL, typed parameter declarations, chart mappings and
refresh/limit settings. Export excludes resolved secrets and result rows by
default; queries may contain sensitive literals, so the export preview includes
their exact definitions. Import validates size/schema, rejects executable
content, requires connection remapping and stays paused until the user reviews
and enables it. Imported definitions never execute during parsing or preview.

Board queries pass SQL classification, engine capability checks and a bounded
read-only execution boundary. Typed parameters are bound rather than spliced.
Stored functions and engine-specific commands require server permissions and
explicit tool validation; a leading SELECT is not a sufficient safety check.
Unsupported read-only enforcement disables automatic query refresh.

Large results reuse Arrow windows. Chart processing is bounded and runs off the
UI thread; downsampling preserves gaps and relevant extrema. Numeric/time/category
mapping is explicit, with table fallback for unsuitable data. Chart descriptions,
keyboard controls, accessible data tables and reduced motion are required.

Reports use a validated application artifact model: title, connection alias,
engine/version, interval, collection status, findings, evidence references,
tables, chart specifications, proposed SQL and limitations. A finding links to
an actual sample, query result or plan. Local deterministic reports work without
AI. AI adds an attributed interpretation and confidence/uncertainty; unsupported
claims never become measured facts. HTML is escaped and charts use validated
specifications, not arbitrary model-provided scripts or markup.

## 6. AI provider setup and existing subscriptions

Settings → AI offers **Existing account/tool**, **API key**, and **Local model**.
Multiple providers can be saved and selected per conversation or scheduled job.
Switching providers reruns the connection's allowlist and sharing checks;
failure never silently sends data to a different provider.

| Choice                    | Planned integration                                                                                             | Status shown to the user                                                                                       |
| ------------------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Codex / ChatGPT           | Official Codex app-server account and turn APIs; use its managed account login rather than copying credentials. | Installed version, authenticated account mode, plan when reported, capabilities and usage limits when exposed. |
| Claude installed tool     | Versioned official CLI/SDK bridge only where third-party use and isolation are permitted.                       | Installed/authenticated/unsupported states and a clear approval or API-key requirement.                        |
| OpenAI API                | Native API adapter with user-selected model.                                                                    | Key configured, verified/unverified, available capabilities and usage.                                         |
| Anthropic API             | Native Messages adapter.                                                                                        | Key configured, verified/unverified, tool/streaming capability and usage.                                      |
| OpenRouter                | OpenAI-compatible adapter with model and permitted downstream routing choices.                                  | Key configured, model/provider capability and routing/privacy policy.                                          |
| Other API/cloud providers | Gemini, approved compatible endpoints, then Azure OpenAI/Bedrock/Vertex adapters.                               | Actual adapter support; unavailable methods stay disabled with a reason.                                       |
| Local models              | Ollama, LM Studio and compatible local servers.                                                                 | Endpoint reachable, model/capabilities, verified local execution or explicitly unknown.                        |

Discovery is opt-in and probes named installed tools or a user-selected absolute
executable. It does not search browsers, enumerate payment subscriptions, scrape
credential files or copy OAuth tokens. Installed is not authenticated;
authenticated is not proof of unlimited entitlement. An unknown plan remains
unknown. Health checks do not send database context or issue a model request.
API keys live in the OS secret store; settings and board files contain only
references. Consumer account usage and API billing are displayed separately.

Provider evidence checked on 2026-10-11:

- Codex documents `account/read`, managed ChatGPT authentication and plan details
  when available: [official app-server documentation](https://learn.chatgpt.com/docs/app-server#authentication-modes).
- Claude's [plan help](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)
  describes subscription usage and the October 7 credit update. Its
  [SDK documentation](https://code.claude.com/docs/en/agent-sdk/overview#get-started)
  separately requires prior approval for third-party products offering claude.ai
  login/rate limits. The subscription bridge remains conditional until that
  product eligibility is resolved; API-key integration is the baseline.
- Claude [bare headless mode](https://code.claude.com/docs/en/headless#start-faster-with-bare-mode)
  skips user configuration but also skips subscription login. An empty working
  directory or an allowed-tools list alone does not establish isolation.
- OpenRouter documents an [OpenAI-compatible API](https://openrouter.ai/developers)
  and [current-key metadata](https://openrouter.ai/docs/api/api-reference/api-keys/get-current-key).
  Downstream provider routing must comply with the connection's allowlist.

Exact adapters, supported versions and vendor rules are rechecked at release.
No integration is labelled Ready until policy, isolation and lifecycle tests
pass. A bridge that cannot deny filesystem/shell/network capabilities outside
the approved provider transport and application tools stays disabled.

### Implemented (M45-01)

API-key and local provider settings are implemented by the Tauri-free
`openrows-ai` crate and lazy Settings → AI pane. The owner explicitly enables
AI network permission (default off), saves multiple keychain-backed providers,
sets a default provider/model and verifies only models/auth/key metadata.
Keys cross only the one-way key-storage command input, never replies/events;
the stricter no-key-in-any-IPC-payload criterion still needs native credential
entry. OpenRouter exposes reported key spend/limit/remaining; other missing facts stay
unknown. Typed model capabilities never imply a paid plan or verified generation
access. Local endpoint reachability does not attest on-device inference.
Cancellation, total deadlines, bounded SSE/NDJSON parsing, full existing TLS,
loopback-by-default local endpoints, warned remote/plaintext opt-in and secret
scrubbing are tested with loopback mocks; no database context or generation IPC
is exposed. [23 §10](23-ai-assistant.md#10-implemented-m45-01-api-key-and-local-provider-settings)
records protocol choices, bounds and manual metadata-only smoke instructions.
Existing-account/CLI eligibility and isolation remain M45-04; per-connection
sharing, routing allowlists and context preview are implemented in M45-02 (§7 below); the
MCP host, query assistant and read validator are implemented (M45-03/05/08), and the monitoring
investigation tools, artifacts and schedules (M45-10..12) are still planned. There is no silent alternate-provider/API-billing fallback.

## 7. Database investigation harness

The harness shares one typed event model across providers: text, tool call,
policy decision, evidence, artifact, usage, completion, cancellation and error.
It builds context, applies policy, calls the provider, validates requested tools,
executes allowed reads, records evidence and returns structured artifacts.
Every step is visible; cancellation kills/reaps local provider processes and
cancels active database reads. Limits cover steps, tokens, cost, time, rows and
bytes; no unlimited autonomous loop is available.

Tools extend [23](23-ai-assistant.md)'s read/propose contract:

| Tool                                           | Result                                                                   |
| ---------------------------------------------- | ------------------------------------------------------------------------ |
| `read_health_snapshot` / `read_metric_history` | Selected, timestamped health evidence and gaps.                          |
| `read_query_statistics` / `read_lock_graph`    | Permitted aggregate query/wait evidence with its scope and visibility.   |
| `describe_object` / `explain_query`            | Relevant schema/index information and a validated plan.                  |
| `run_read_query`                               | Bounded, authorised read with columns/results filtered by policy.        |
| `propose_sql`                                  | Classified SQL in an editor proposal; no execution of writes.            |
| `propose_board` / `build_report`               | Validated, previewable board/report artifacts tied to existing evidence. |

Health telemetry gets its own per-connection sharing switch; schema-only policy
does not implicitly permit activity text, users, client addresses or timings.
None/SchemaOnly/SchemaAndStats/SampleRows, telemetry permission, redaction and provider
allowlists are checked before every outbound payload and tool response.
Policy changes or app lock revoke active work, not only the next conversation.
Database comments, rows, imported boards and provider output are untrusted
content; they cannot override policy, create tools or change the chosen provider.

Scheduled investigations are explicitly enabled per connection, with a chosen
provider, permitted tools, recurrence/event trigger and spend/step limits.
Sampling and deterministic alerts never call a model on every polling tick.
Automatic AI analysis requires a pre-authorised sharing policy and remains
read-only; findings, reports and proposed fixes are reviewed in the app. Audit
records contain provider/model, scope, tool names, evidence ids, usage and outcome,
with bounded retention and no keys or raw payloads by default.

### Implemented (M45-02)

The common Tauri-free `openrows-ai::sharing` boundary defaults every connection
to None, requires explicit production consent, saved-provider allowlisting and
optional loopback local-only routing. SchemaOnly never includes rows/statistics;
SchemaAndStats permits typed aggregate/plan context only with separate telemetry
consent; SampleRows adds bounded redacted samples. Raw activity identities,
addresses and arbitrary vendor plan properties have no field in this bundle.
Shared SQL lexing removes literals/comments; identifier and per-column rules,
row/cell/byte/token caps and exact offline preview apply at every outbound gate.
Private non-deserializable gated payloads are required by provider requests;
configuration changes/revocation invalidate them and cancel active transport.

Lazy Settings → AI → Data sharing and per-connection editors expose the controls,
redaction rules, exact rendered context/sizes and a local metadata audit viewer
with confirmed purge. Policies/audit use separate private device storage; neither
is exported with preferences or connections. Audit appends must succeed before
sending, retain the newest 1,000 entries and never include payloads or keys.
Golden/refusal/compile-fail, transport/audit/persistence and UI tests are linked
in [23 §11](23-ai-assistant.md#11-implemented-m45-02-connection-context-sharing).
The settings preview accepts selected SQL/definitions offline; later tool and
assistant producers supply typed plans/statistics/samples through the same API.
M45-05 now owns query conversations (§7 below); M45-10 owns monitoring investigations.
M45-02 itself adds no live database collection or generation command.

### Implemented (M45-03/08)

The built-in MCP host now exposes seven bounded catalog/read/plan/proposal
tools through stdio and optional loopback HTTP; tokens are provider/connection
scoped, expiry checked and stored only as hashes. Every disclosure uses the
M45-02 gate and content-free audit before release. Read tools share a conservative
SQL validator and disposable engine-protected execution boundary; proposed
writes only open ordinary editor tabs. Lock, AI-off, token/sharing revocation
and connection close cancel active work. Catalog cancellation discards the
metadata connection. Static tool instructions keep hostile database data out
of the control plane. PostgreSQL, MySQL/MariaDB, SQLite and libSQL file fixtures
verify unchanged state after smuggling and an injected engine-level write.

[23 §12](23-ai-assistant.md#12-implemented-m45-0308-built-in-mcp-and-execution-safety)
records exact bounds, conservative SQL/engine restrictions and Claude Code/Codex
setup. Query conversations and tool-call traces are implemented below; monitoring
collectors/scheduling, boards/reports and structured evidence artifacts remain M45-10.

### Implemented (M45-05)

The query-tab assistant reuses MCP's `ToolExecutor` for catalog, protected reads,
plan-only explain and proposals. `openrows-ai::agent` bounds provider/tool rounds,
streaming/backpressure, steps, conservative tokens and total time; cancellation
and live policy revision checks cover transport and tools. Every call has a visible
start/completion trace with safe argument summary, result size/redaction, duration
and status. Provider output has editor proposal actions and no write execution path.
Unknown tool capabilities fall back to one gated schema prompt. Native resolved
scope, questions, re-redacted prior turns and optional typed plan go through the
same producer for exact offline preview and generation.

AST admission now permits read-only joins/subqueries/non-recursive CTEs,
aggregates/windows and UNION for PostgreSQL, MySQL/MariaDB and SQLite/libSQL file.
Every base relation is live-validated; unknown functions remain refused on every
engine and ambiguous sample lineage is fully redacted. Existing disposable
read-only sessions, deadlines and no-startup-SQL behavior remain mandatory.
[23 §13](23-ai-assistant.md#13-implemented-m45-05-query-assistant-with-visible-tool-trace)
records bounds, tests, engine decisions and opt-in `OPENROWS_AI_LIVE=1` smoke.

Query conversations/traces are session memory only. Persisted audit remains the
M45-02 content-free disclosure metadata; richer model/usage/evidence/outcome
records, monitoring tools, scheduled investigations and artifacts remain M45-10/12.

## 8. Acceptance and verification

1. Monitoring remains useful with no AI provider; observed server activity,
   counters, blocking and recovery match controlled integration fixtures.
2. Refresh is bounded, never overlaps, and never blocks the user's query.
   Duplicate views share collection. Close/lock/disconnect/revoke cancels work;
   wake, restart and statistics reset produce gaps rather than false rates.
3. Every supported engine has real collector tests and explicit capability and
   permission states. DuckDB never opens an extra writable host/session.
4. History retention, size caps, deletion and profile isolation are verified.
   Alert onset/recovery/deduplication use controlled samples and clock tests.
5. Charts and board parameters reproduce fixture results; import does not run
   SQL. Cancellation, read-only enforcement, query limits and hostile input are
   tested at the execution boundary, not just by UI disabled buttons.
6. Reports/charts export with source/time/scope/limitations, and preserve missing
   data. Invalid chart mappings, script content and unsupported evidence fail
   typed. Keyboard navigation and accessible table alternatives work.
7. Provider setup covers missing, signed-out, unsupported, verified and unknown
   states. OpenAI/Anthropic/OpenRouter streaming/tools/errors and local models
   pass recorded fixtures; supported account bridges pass lifecycle/isolation
   tests. Missing provider approval is a visible unavailable capability.
8. Golden payloads verify sharing/redaction across API, local and CLI paths.
   Prompt injection, write SQL, forbidden functions and revoked policies cannot
   escape the read/propose boundary. No credentials or sampled literal/parameter values appear in monitoring exports, logs or provider payloads; collector redaction remains mandatory before sharing.
9. The assistant can investigate an incident, obtain allowed evidence, propose
   SQL, and produce a validated chart/report with a visible trace. Provider
   failure/cancellation is typed; limits and audit records are enforced.
10. A user-enabled scheduled investigation obeys budgets, sharing and lock
    state; paused/disabled schedules do no work. No closed-app service or remote
    notification is implied. Provider/model evaluations measure query result
    correctness, evidence grounding and unsafe tool rejection.

Run the repository gates and engine integration suites for each implementation
slice. Timing/overhead, retained-history footprint and UI/chart performance are
measured against [13](13-performance.md) at milestone exit, with reference
hardware and actual numbers. Neither this specification nor the existing
process-list refresh counts as implementation evidence. M25-09 must meet all accepted ADR-0011/NFR-PERF-11/NFR-SEC-08 budgets and the detailed source/privilege, advisor, Explain, privacy and retention matrix; M45-09 separately validates AI sharing and tool boundaries.


## 9. Implemented (M25-01/02): monitoring runtime and PostgreSQL backend

`openrows-monitor` owns the Tauri-free actor, typed capability/sample states,
monotonic counter baselines, cadence/deadlines, blocking analysis and redacted
Arrow windows. PostgreSQL owns fixed guarded reads and version/role/extension
probes. The desktop admits an explicitly started collector only for an already
connected PostgreSQL profile/database; startup commands are removed from the
background-session route. TLS, credentials and SSH/helper routing reuse the
existing connection path. Repeated views share the same reservation/session;
up to four pending/active collectors count against admission.

Activity defaults to 5 s; database/statements/I/O have a 15 s floor, and
relations/indexes/replication a 60 s floor. Slow, failed or unchanged sources back
off to 60 s. Reads run sequentially, with a client deadline plus PostgreSQL
read-only transaction, local `pg_catalog` search path, statement timeout and
100 ms lock timeout; no
transaction spans ticks. A client/protocol failure cancels and discards the
session rather than reusing an undrained connection. Explicit resume opens and
re-probes a new generation. Stop/pause, connection close, profile edit/delete,
network/wake invalidation, app lock and application exit stop and clear samples;
unlock never resumes collection implicitly. Revoked statistics visibility is
rechecked inside each data query and clears affected source data.

The live store keeps at most ten minutes, 300 timestamps/series, 64 scalar
series and 200 statement identities. The implementation uses a stricter 2 MiB
retained estimate per collector, explicit partial coverage on caps, and bounded
one-MiB Arrow pages (up to 1,000 rows; top statements default 20/max 100).
Read-time expiry also removes history after terminal collection failure.
Events contain only generation/source status, at most once per second. Query
literals/comments/parameters are removed before retention; utility, quoted
identifier, multi-statement and truncated text fails closed. M25-03 extends
statement reads with bounded normalised text from
`pg_stat_statements(true)`; core removes literals before Arrow/retention and
unsafe text still fails closed.

Typed `monitor_start/stop/pause/capabilities/snapshot`, `monitor_window` and
`monitor_top_statements` commands, `monitor-changed`, generated types and the
`packages/ipc/src/monitor.ts` helpers form the M25-03 handoff. Existing Processes
reads the redacted monitor activity when a collector is registered, without a
second activity query. Sample nulls carry explicit unknown/gap/reset states;
server and selected-database scopes, units and setup guidance accompany samples
and capabilities. UUID aliases share one profile admission/generation key.
Statement rankings currently cover the latest valid interval; cumulative mean
and max-since-reset stay separately labelled. Full-window rankings were added
by M25-04 (§11). Statement coverage includes collector/guard commands where their query
identities cannot be excluded safely.

Evidence and limitations are recorded in [ADR-0011](adr/0011-performance-monitor.md#implemented-m25-0102-evidence).
Native-arm64 PostgreSQL 17 and 14 full suites and the PG12 monitor fixture cover
active/blocking work, positive counter rates, a real database-statistics reset,
extension absence/presence and permission loss. PG12's unrelated multirange test prevents a green full PG12
suite. PG13/15/16/18, managed-provider denial, unloaded extension, live replication,
spill/vacuum/statement-reset fixtures, the mac-low all-process footprint and
reference-workload overhead matrix remain unverified. No monitor UI, local
history, alerts, boards, advisor or AI harness is shipped by this backend slice.
The final serial Rust run passed 1,389 tests; the full gate remains incomplete
because the two-container SSH fixture conflicts with this task's one-container
constraint. Initial host cancellation failures passed on that final rerun.


## 10. Implemented (M25-03): Monitor workspace and local history

PostgreSQL fixture synchronization waits for actual execution: process cancellation
requires the target's active `Timeout: PgSleep` event, since Parse/Describe can
already publish its SQL. Listing failures end the joined execution immediately.
The blocking fixture keeps the lock until a fresh activity sample shows the same
waiter active, waiting on a lock, and naming the exact blocker PID; activity state
is transaction-cached while wait events and blocker lookups are live. Regression
tests cover early SQL visibility and idle state paired with live blockers.

Final M25-03 quality evidence: workspace Rust `-E 'not binary(ssh_container)'`
passed 1,455 tests (198 skipped); Vitest passed 2,766 tests across 243 files.
Formatting, Clippy/ESLint, TypeScript, licence/dependency scans, hygiene, UI
tokens/colours, bundle budgets and IPC freshness all passed.


A lazy **Monitor** utility tab opens for the connected profile/database from the
command palette, native menu and connection/database context menus. It offers
Overview, Sessions, Blocking, Top queries, Tables, Indexes, Replication and I/O.
The header shows scope, live/paused state, sample age and Off/2/5/10/30/60-second
controls; core continues enforcing the 15/60-second source floors. Closing,
disconnecting or locking stops collection and disposes the Arrow worker;
unlocking requires explicit resume. Later slices admit every engine (§12–§16, §19).
The existing Processes tab and confirmed cancel/kill actions remain in use.

Eleven scalar tiles use bounded SVG trends with gaps for unknown, missing,
permission and reset samples. Keyboard-readable data tables are mounted only
when opened. Source panels expose scope, effective cadence, sample latency,
last success, partial coverage and copy-only privilege/extension guidance.
Sortable source grids page 100 rows; the blocking tree bounds depth and visible
nodes and identifies missing owners/cycles. Statement rankings show covered
latest-interval deltas and separately labelled cumulative statistics. Copy and
editor actions use normalised redacted SQL; plan-only Explain first requires
explicit statement/binding review and reuses the existing safety boundary.
Full-window rankings and deterministic advice are implemented in §11; optional HypoPG remains unavailable.

Settings → Monitoring and the tab's history controls enable collection storage
per profile, default off. SQLite migration 0007 stores only allowlisted scalar
aggregates, scoped by profile/database, with no statement text, activity rows,
user identities or credentials. Expiry runs on startup/read/write/configuration.
A conservative 2-KiB charge per bounded row limits retained history to 50 MiB
per connection and 100 MiB app-wide, evicting oldest samples first. SQLite may
reuse freed pages; purging does not shrink the shared store file. Disable purges
atomically; per-profile/all purge and profile deletion remove history. Opt-ins
are device-local and excluded from portable preferences. History ranges are
1/6/24 hours, at most 300 buckets per metric, preserving missing states; prior
window values are compared when retained coverage exists. The store is not
encrypted and database scope names may be sensitive.

Development-machine evidence (Apple M4, 24 GiB, native arm64 PostgreSQL 17.11):
`OPENROWS_BENCH=1 just test-pg 17` passed all 79 tests. A workload and blocker pair
produced three sessions, `Lock: transactionid`, 591.55 transactions/s,
4,197.18 rows read/s, one table and 53 statement rows in the actual UI components
using captured, core-redacted Arrow pages. Source read plus guards p95 was
9.824 ms; 1,000-row analysis p95 was 4.361 ms. Chromium development rendering of
11 charts × 300 timestamps over a ten-minute window measured 8.3 ms p95,
9.3 ms maximum, 16.7 ms frame p95, and no >50 ms reports from the existing dev
monitor. This is component/Chromium evidence; native WKWebView and mac-low
reference hardware budgets are not measured yet (§20). The first PG run failed in an
unrelated custom-hop test and passed on an unmodified rerun.

Vitest covers source and engine capability states, gaps/reset validity,
interval/pause/resume, close/disconnect/lock and late admission, review-only
Explain, accessible chart tables, default-off history, settings persistence and
purge. Rust tests cover profile/database isolation, startup/age expiry,
retention caps, raw-text rejection and gap-preserving bucket limits. No new
dependency or chart library was required; SVG avoids a large plotting bundle.
The lazy production build reports 305,167 gzip bytes of initial JavaScript
(budget 350,000) and 6,306 gzip bytes of shared UI CSS (budget 15,000).

## 11. Implemented (M25-04/10): PostgreSQL investigation and advisor

Top queries rank by summed execution-time deltas or weighted mean over retained
live intervals. Detail shows normalized SQL, actual interval duration, calls,
time, returned/affected rows, shared buffer hits/reads and temporary block I/O.
The source maximum stays labelled “since reset”; PostgreSQL aggregate statement
statistics do not supply p95/p99, per-query CPU or query memory. They remain
unavailable. Related sessions match normalized SQL within the selected database;
that is an approximate cross-source association, not an application call trace.
Blocking chains include statements, query/transaction ages and wait events with
a link to the exact session. Graph caps, cycles and unknown owners remain visible.

Live statement history is bounded to ten minutes, 41 observations per identity,
200 identities and 512 KiB inside the existing 2 MiB live-store cap. Unknown
baselines, resets, disappeared/reappeared identities, partial snapshots and
permission loss cannot contribute fabricated deltas. Window mean is summed
execution time divided by summed calls. Regression version 1 compares adjacent
complete intervals with at least five calls each: mean must rise by at least
2× **and** 10 ms. End minus actual duration must match the preceding end within
1,000 ms (strictly less); gaps cannot trigger a regression. Detail marks limited coverage.

The Tauri-free advisor contains deterministic PostgreSQL rule version 1:

| Rule ID | Required evidence / threshold | Proposal |
| --- | --- | --- |
| `pg.selective-scan` | Explicit Seq Scan on ≥100,000 estimated table rows, ≤1% estimated output, simple equality key, complete fresh index absence | Quoted `CREATE INDEX CONCURRENTLY`; medium confidence |
| `pg.join-key` | Explicit large sequential scan, resolvable equality join key without a usable leading btree index | Concurrent index candidate; low confidence, hash/full scan may be optimal |
| `pg.estimate-error` | Explicit measured node, positive loops, ≥10× estimated/actual mismatch or exactly one zero | Quoted `ANALYZE`; no claim it fixes skew/correlation |
| `pg.stale-statistics` | Known analyze age ≥24 h, ≥500 modifications and ≥10% estimated live tuples | Quoted `ANALYZE` |
| `pg.plan-spill` | Actual measured plan node reports disk spill | Prose about query/index review and session memory concurrency; no `SET` |
| `pg.statement-spill` | Positive temporary-block read or write delta in a valid interval | Same prose; blocks are not bytes |
| `pg.unused-index` | Zero cumulative scans, known database reset age ≥7 days, valid ordinary btree; excludes PK/unique/constraint/replica identity | Concurrent drop; low confidence and creation-age, seasonality, replica/reset caveats |
| `pg.duplicate-index` | Matching parent, ordered keys/includes, predicate, expression, uniqueness, access method, opclasses, collations and ordering signature | Drop only one non-protected equivalent index concurrently |
| `pg.overlapping-index` | Ordered simple key prefix | Inspect definitions; no drop SQL from a prefix alone |
| `pg.vacuum` | ≥100,000 estimated dead tuples, ≥20% dead/(live+dead), known vacuum age ≥24 h, ≥3 complete observations in one reset epoch | Ordinary `VACUUM`; low confidence, no measured byte bloat or file-shrink promise |
| `pg.batching` | ≥1,000 calls/min, mean ≤10 ms in three contiguous complete intervals | Text-only batching/caching investigation; does not prove request-level N+1 |
| `pg.lock-hotspot` | Same statement/blocked edge in ≥3 complete activity snapshots | Inspect transaction/access order; no cancel SQL or inferred wait duration |

Catalog/activity evidence expires after 120 s and explicit plan evidence after
ten minutes; future, incomplete, missing or denied sources suppress dependent
rules. Malformed catalog identities, key arrays or usability flags also make
absence-based evidence incomplete. Plans resolve relations only against complete
fresh catalog metadata. Only eight bounded literal-free plan projections are retained. Index
proposals require a fresh complete index snapshot; a newly observed leading
index suppresses a prior scan candidate. Cards have stable IDs, rule versions,
confidence, exact values/node identities, collection time, qualitative impact
and workload/locking/storage caveats. At most 64 cards are returned in stable
impact/identity order. There are no provider calls or automatic server changes.

The Monitor advisor and per-query list use the existing proposal queue to open
SQL with evidence/caveats in an ordinary editor tab. Native admission reevaluates
the card against current evidence; expired or changed cards cannot enqueue stale
SQL. Running an editor proposal follows ordinary ReadOnly, confirmation,
authentication, production and app-lock policies. Dismissal/snooze stores only
bounded card identities and expiry times, scoped by profile/database; no SQL or
sample data is persisted for these preferences.

Explain opens a review sheet to supply complete bindings and schema. Plan-only
is the default. A separate disposable PostgreSQL diagnostic session excludes
profile startup SQL and user transactions. Native SQL/context/mode/policy-bound
single-use confirmation is required for every measured run, including SELECT; #99
ReadOnly, OS authentication, lock admission and the production warning still
apply. Setup uses only session search path, standard strings and bounded
statement/lock timeouts; ReadOnly profiles also set a read-only transaction default
on this disposable session. Other profiles preserve the server's transaction default. One diagnostic per monitor, connection/execution
deadlines, a two-second owned-transport shutdown deadline, cancellation and monitor-stop signalling bound its lifetime.
Rollback does not undo sequences or external functions. Raw plan/predicates and
dynamic detail values are removed before monitoring IPC; the existing visualiser
still displays numeric plan hotspots. Reviewed SQL must normalize to the
selected sampled statement, preventing unrelated evidence attribution.

Reviewed diagnostics reject unresolved positional/named/anonymous placeholders.
The standalone PostgreSQL `?` JSONB operator is conservatively rejected as
ambiguous with a redacted binding; ordinary editor Explain remains available for
that expression. Question marks inside strings/comments and `?|`/`?&` operators
are accepted by the existing SQL lexer.

Optional HypoPG what-if is off until explicit opt-in for a reviewed statement
and index proposal. Native admission rechecks sampled-statement identity,
current index evidence, profile policy, authentication and app lock. When the
extension is already installed, a separate bounded session compares plan-only
estimated costs before and after `hypopg_create_index`, calls `hypopg_reset`
and closes. Absence, denied creation and successful cleanup have native
mock-session tests. This never installs HypoPG, creates a real index or claims
measured improvement.

Known gaps: persisted per-statement investigation history is not added; the
existing default-off local history retains scalar series. Other engines have
collectors (§12–§16, §19) but no advisor rules, and Explain/HypoPG from the
Monitor are PostgreSQL only. Reference-workload/mac-low certification is listed
as not measured in §20. A server with HypoPG installed was unavailable for a live what-if
fixture; the isolated path has mock-session coverage.

## 12. Implemented (M25-07a): MySQL and MariaDB collectors

The lazy Monitor admits both engines through the existing dedicated-session
route, without replaying profile startup SQL. Distinct version layouts select
MySQL performance-schema process/lock tables or MariaDB Information Schema
processes and InnoDB lock waits. Queries are fixed read-only projections, with
server statement deadlines, client cancellation, packet/row/byte caps and
uptime/decreasing-counter reset detection. No instrumentation, global setting,
grant or restart is performed by monitoring.

Server-scoped status supplies query rates, connected/running threads, available
InnoDB row/cache counters, temporary disk tables, slow queries, aborted connects
and deadlocks. MariaDB reports `Innodb_deadlocks`; MySQL has no such status
variable, so it reads the enabled `INNODB_METRICS.lock_deadlocks` counter when
PROCESS and Performance Schema allow it. Missing counters stay unknown. InnoDB hit ratio uses interval
`1 - buffer_pool_reads / buffer_pool_read_requests`, distinct from PostgreSQL.
Digest/schema/first-seen identity bounds statement statistics and converts
picoseconds to milliseconds once; all SQL still passes core redaction. SQL modes
with ambiguous quote/backslash semantics yield fingerprint/metrics only.
Table/index I/O and file I/O use instrumented summaries; both engines optionally
join `sys.schema_unused_indexes` and `sys.schema_redundant_indexes` when the
views probe successfully (MariaDB ships `sys` from 10.6). Zero reads are never a
drop recommendation. Replica status retains named state/lag only,
discarding server addresses/error text; stopped/NULL/no-replica remain distinct.

Descriptors distinguish disabled Performance Schema (DBA startup configuration
and restart required), disabled timing/consumers and missing SELECT/PROCESS/
replication grants. Without PROCESS both engines silently list only the
caller's own threads, so discovery probes `INNODB_TRX` and reports a
permission-denied activity source naming PROCESS instead of an empty success.
Non-InnoDB/metadata blocking, per-query CPU/memory and percentiles remain
unavailable; MySQL blocking needs Performance Schema (`data_lock_waits`), while
MariaDB reads `INNODB_LOCK_WAITS` without it. InnoDB caches its transaction and
lock-wait tables and refreshes them only after 100 ms without readers, which the
default 5 s cadence always satisfies.

The Monitor tab labels MySQL/MariaDB table, index and file sources as Table I/O,
Index I/O and File I/O, and the overview shows server-scoped tiles (queries/s,
threads, InnoDB buffer pool hit ratio, InnoDB rows, temporary disk tables, slow
queries, aborted connects, deadlocks, applier lag). Integration fixtures on
native-arm64 MySQL 8.4 and MariaDB 11.4 (Performance Schema on, plus MariaDB's
default off) and the opt-in overhead benchmark are recorded in ADR-0011. Wider
live version/provider and reference-load/all-app performance validation is not
measured yet (§20).

## 13. Implemented (M25-05): incidents and background monitoring

`openrows_monitor::alerts` is a Tauri-free, model-free rule engine. A rule
names a live aggregate (for example `sessions:waiting`,
`Database:cache_hit_ratio` or `Replication:replay_lag_ms`), a direction, a
firing threshold and a separate recovery threshold. Validation rejects
non-finite values, a recovery threshold on the unhealthy side (no hysteresis
band), zero-sample holds, durations or cooldowns above one hour and duplicate
ids. An incident opens only after the breach holds for both a minimum number of
consecutive known samples and a minimum wall-clock duration. It resolves only
after recovery holds in the same way. Values inside the hysteresis band neither
resolve an open incident nor count toward a new one. After recovery, a cooldown
must pass before the same rule and scope can reopen. There is at most one open
incident per rule and scope (server or database), and repeated or out-of-order
timestamps are ignored. Unknown, gap, reset, unavailable, permission-denied or
non-finite samples interrupt both streaks. They can never open or resolve an
incident. An incident records its rule snapshot, scope, onset time, up to eight
onset values, peak, latest or recovery value and end time. The engine retains 50
incidents per target and evicts resolved ones first.

Built-in rules cover waiting, idle-in-transaction and active sessions, the
PostgreSQL cache and InnoDB buffer-pool hit ratios, deadlocks and replica replay
lag. They are evaluated wherever the engine supplies the metric. Deadlocks fire
on the first observed deadlock interval. The other rules need three samples and
30 s to fire or recover, with a 60 s cooldown. Rule editing is not exposed yet.

The desktop evaluates the latest live observations on each collector status
event. Evaluation runs only for a running collector while the app is unlocked,
so a stopped, paused or locked monitor does no alert work. Incidents stay in
memory for the app run (at most 16 targets) and are never persisted or sent
anywhere. `monitor-incident` events drive an in-app notice, which is cleared and
hidden while locked. The Monitor's **Incidents** panel lists open and resolved
incidents with rule, scope, start/end and evidence, plus the active rules.

Background monitoring is an explicit per-connection opt-in (**Background** in
the Monitor header) for a running collector. With it on, closing the last
Monitor view keeps that collector and its rules running while OpenRows is open.
There is no OS service, login item or daemon. Pause, lock, disconnect, profile
removal, network re-probe and app exit stop the collector and clear the opt-in.
After unlocking, the user must reopen the Monitor; nothing restarts
automatically. A cadence change keeps the opt-in across the restarted
generation.

## 14. Implemented (M25-07b): SQL Server collector

The lazy Monitor admits SQL Server through the existing dedicated-session route,
without replaying profile startup SQL. Discovery reads the product version and
engine edition from the session summary and accepts SQL Server 2017+, Azure SQL
Managed Instance, Azure SQL Edge and Azure SQL Database; Synapse and Fabric stay
unavailable. One `HAS_PERMS_BY_NAME` probe checks VIEW SERVER STATE (plus VIEW
SERVER PERFORMANCE STATE on 2022+) or, on Azure SQL Database, the database-scoped
pair. Without it SQL Server silently lists only the caller's session, so every
DMV source is reported as permission-denied naming the permission instead of an
empty or zero result. Each permitted source is then probed once; a missing DMV,
column or function (errors 195/207/208/4121) is unavailable for that platform.

Every read is one fixed batch: `SET LOCK_TIMEOUT 100`, an epoch declaration and a
bounded projection. At three quarters of the deadline the attention signal
cancels the batch and the session stays usable when the server acknowledges in
time; otherwise the reading is terminal. Rows, cells and bytes are admitted
before text allocation under the shared monitor caps.

- **Sessions/blocking:** user sessions joined to requests, the current or most
  recent statement text, transaction age from the transaction DMVs and one
  blocker per request from `blocking_session_id`; the core builds chains.
- **Waits** (Waits panel, I/O slot): top 200 non-idle `sys.dm_os_wait_stats`
  rows (`sys.dm_db_wait_stats` on Azure SQL Database) with tasks, wait and
  signal-wait counters. The server start time is the epoch; a
  `DBCC SQLPERF(..., CLEAR)` keeps the epoch and the decrease becomes a counter
  reset rather than a negative rate.
- **Resource evidence:** whitelisted `sys.dm_os_performance_counters` pivoted to
  one server row (batch requests, compilations, re-compilations, user
  connections, blocked processes, lock waits, deadlocks, page life expectancy,
  total/target server memory and pending memory grants). Buffer cache hit ratio
  and default resource pool CPU divide their documented base counter at read
  time; a missing counter stays unknown.
- **Top queries:** `sys.dm_exec_query_stats` grouped by `query_hash` for plans
  whose `dbid` plan attribute is the selected database, top 200 by elapsed time,
  with calls, elapsed/CPU milliseconds, rows and logical/physical I/O. Plan-cache
  eviction lowers the sums, which resets those rows' baselines.
- **Table and index usage:** `sys.dm_db_index_usage_stats` seeks, scans, lookups
  and updates. Indexes unused since instance start or database open are absent,
  which is never treated as proof that an index can be dropped.

Replication (Always On availability groups) is explicitly unavailable, and Query
Store history is not read yet. The UI shows SQL Server counter tiles and labels
the table, index and I/O panels as Table usage, Index usage and Waits. The
integration fixture runs against native-arm64 Azure SQL Edge (15.x): blocking
chain, cleared wait statistics, counters, top queries, usage and a login without
VIEW SERVER STATE. Per-read server time on that fixture is about 8 ms for
sessions, 15–18 ms for top queries, 5 ms for usage and 2 ms for waits. Live SQL
Server 2022, Managed Instance and Azure SQL Database validation is not measured
yet (§20).

## 15. Implemented (M25-07c): ClickHouse collector

The Monitor admits ClickHouse through the existing dedicated-session route. It
uses the same lazy start and replays no profile startup SQL. Every collector
statement is a fixed read-only projection. Its trailing `SETTINGS` clause sets
`max_execution_time`, result and read caps with `throw` overflow, and then
`readonly = 1`. A read-only profile or a settings constraint that rejects these
guards makes the source unavailable, so no source ever runs unbounded. A client
deadline and `KILL QUERY` cancellation also apply. Each response is admitted up
to 1 MiB and 1,000 rows. Cells of 4 KiB or more become NULL and mark the reading
partial. The collector excludes its own queries by query-id prefix. It never
flushes logs, changes settings or grants access.

| Source (panel) | ClickHouse read | Notes |
| --- | --- | --- |
| Activity (**Running queries**) | `system.processes` | Elapsed time, memory and peak memory, rows/bytes read and rows written, initial or secondary query, node name. There are no blocking chains or waits. |
| Database (overview) | `system.events`, `system.metrics`, `system.asynchronous_metrics` | Whitelisted event counters become rates. Running queries/merges, connections, tracked memory, read-only replicas, parts and replica-delay values are gauges. Zero-valued events are listed (`system_events_show_zero_values`). A decrease in `uptime()` resets baselines. A denied table leaves its counters unknown and is named in coverage. |
| Statements (**Query log**) | `system.query_log` | Only `QueryFinish` and `ExceptionWhileProcessing` events, with no `QueryStart`. Rows are grouped by database, `normalized_query_hash` and initial/secondary query, inside an hourly window pruned by `event_date` and `event_time`. The window start is the counter epoch, so each hour begins with one reset interval. The text is `normalizeQuery` output and still passes core redaction. |
| Tables (**Parts**) | `system.parts` (active parts) | Parts, partitions, maximum parts per partition, rows, on-disk, compressed and uncompressed bytes per table in the selected database. |
| I/O (**Merges**) | `system.merges` | In-progress merges and mutations on the node. |
| Replication | `system.replicas` | Per-table state, queue sizes and `absolute_delay` as `replay_lag_ms`. No replicated tables is "not applicable", never a zero lag. |
| Indexes | — | Unavailable: ClickHouse has no index usage statistics. |

A missing `system.query_log` is reported as unavailable. This covers both a log
that is not configured and one that has not flushed yet; ClickHouse checks this
before grants. A sampled log (`log_queries_probability < 1`) is noted in
coverage. ClickHouse error 497 (`ACCESS_DENIED`) and authentication failures
become permission-denied states that name the grant to review. Deadline,
read/result-cap, memory and cancellation errors are gaps; a cap also marks the
reading partial.

The scope is explicit. By default every source reads the connected node only,
and each coverage text starts with `Node scope: the connected node <host>
only.` The profile option `monitor_cluster=<name>` (for example
`?monitor_cluster=prod` in the connection URL) requests cluster scope. It
applies only to running queries, and only when a probe of
`clusterAllReplicas('<name>', system.one)` succeeds. The name must be a plain
identifier, the cluster must exist and REMOTE must be granted. Other sources
remain on the node. If the probe fails, the coverage names the reason and keeps
node scope.

The overview shows node tiles: running queries, queries/s, failed queries/s,
selected and inserted rows/s, delayed or rejected inserts/s, running merges,
maximum parts per partition, tracked memory, connections, read-only replicas
and replication delay. No cache-hit ratio is invented. The built-in
active-session and replica replay-lag alert rules apply where those metrics
exist. The Blocking and Indexes panels are hidden for ClickHouse. The Processes
view keeps its own listing, because ClickHouse query ids are strings mapped to
per-listing tokens. Local history accepts the new ClickHouse aggregates.

Fixtures run on native-arm64 `clickhouse/clickhouse-server:25.3.9.72` with
`select_from_system_db_requires_grant` on (`scripts/test-clickhouse.sh`). They
cover counters (event advance, listed zero events, gauges), a running query,
parts/merges, query-log top queries with normalised literals, cluster scope
over the `default` cluster, and a restricted user. That user has
`system.processes`/`system.events` only, plus a requested cluster without
REMOTE. A second run with `OPENROWS_TEST_CLICKHOUSE_QUERY_LOG=OFF` removes
`query_log` and checks the unavailable state. A live multi-node cluster,
ClickHouse Cloud and reference-load overhead are not measured yet (§20).

## 16. Implemented (M25-07d): SQLite and libSQL own-session diagnostics

SQLite, libSQL files and remote libSQL are admitted to the Monitor. Neither
engine has a server process list, so every collector sees only what OpenRows
itself runs, and the Overview states the ADR-0011 boundary in plain text: “Own
OpenRows sessions only — other applications are not visible.” for files and
“Own OpenRows requests only — server-wide metrics unavailable over Hrana.” for
remote libSQL. Sessions, Blocking, Tables, Indexes, Replication and I/O panels
are hidden for these engines; their sources report an explicit unavailable
reason, never zero.

- **Own statements** (Own queries panel): a process-wide ledger per file path
  (canonicalised) or remote origin and path (never credentials) is shared by
  every OpenRows session on that target. Statements run through the editor or
  grid (`execute` / `execute_bound`) are recorded after they finish; catalog,
  editor, connect and monitor probes are not. Identity is an FNV-1a hash of the
  literal-free monitor normalisation; utility statements group by keyword
  (`other:PRAGMA`) without text. Each shape keeps calls, client wall time,
  maximum, rows, errors and, for files, `sqlite3_stmt_status` FULLSCAN_STEP,
  SORT and VM_STEP. At most 200 shapes are kept; the cheapest is evicted and
  the reading is marked partial. The ledger ends with its last session, and the
  next one starts a new counter epoch.
- **File diagnostics** (SQLite and libSQL file, Database source): the monitor
  connection waits at most 100 ms for a writer and runs only `PRAGMA
  page_size`, `page_count`, `freelist_count` and `journal_mode`, reads the file
  and `-wal` sizes, and reads the documented WAL-index header in `-shm`
  (`mxFrame`, `nBackfill`) as WAL frames and checkpoint backlog. A torn or
  uninitialised header, a rollback-journal file or an in-memory database leaves
  those values unknown. No checkpoint, `integrity_check`, `quick_check` or
  `dbstat` scan runs on a tick. Own request totals (requests, errors, busy
  milliseconds and rows) become per-second rates.
- **Remote libSQL**: a timed `SELECT 1` on the monitor stream (aborted at three
  quarters of the deadline) plus own request totals. Server CPU, memory,
  storage, waits, replication and other clients' queries are unavailable over
  Hrana and are never estimated; a refused token is an explicit denied state.
  Hrana response timings (`query_duration_ms`) are not read yet.

The Overview shows own-request rates and, for files, file size, pages, free
pages, WAL size, WAL frames and checkpoint backlog. History keeps only the
reviewed scalars (`page_size`, `page_count`, `freelist_count`, `file_bytes`,
`wal_bytes`, `wal_frames`, `checkpoint_backlog`, `requests`, `request_errors`,
`request_ms`, `rows_returned`, `round_trip_ms`); journal mode and statement text
never persist. Tests cover the ledger, WAL-index parsing, a tempfile SQLite file
in WAL and rollback modes, and native-arm64 sqld v0.24.32 for remote libSQL.
`sqlite3_db_status` cache counters belong to the user's connection rather than
the monitor's and are not collected yet.

## 17. Implemented (M25-06): analytics boards

The connection's Monitor tab has a **Boards** panel. Boards are stored per
profile in the local store (migration 0008, table `boards`) as the validated
board JSON plus a `reviewed` flag. A board has typed parameters, an optional
refresh interval (30 s, 1, 5 or 15 min) and up to 24 widgets. Each widget is a
chart (line, area, bar or scatter, with an explicit x column and 1 to 8 numeric
y columns), a stat card (first cell) or a table. Each widget holds exactly one
statement. The statement must classify as a read in the profile's dialect,
which is checked on save, import and every run.

Parameters are `text` (at most 1,000 characters), `number` (finite decimal),
`date` (`YYYY-MM-DD`) or `enum` (declared options). SQL names them as `:name`.
The core resolves each placeholder against the declared parameters. It checks
the entered value or default against the type and rewrites the placeholder into
the engine's bind placeholder. Values travel only as bound parameters.
Undeclared names, missing values and mistyped values are refused before a
connection opens.

A widget runs only on an explicit **Run**. Opening, importing or approving a
board runs nothing. The native `board_run` command loads the definition from
the store, so the webview cannot substitute SQL. It refuses an unreviewed board
and is admitted through the app lock. Each run opens a disposable connection to
the connected profile without its startup SQL, applies the engine's protection,
runs the bound statement into a bounded collector, then rolls back and closes.

| Engine | Protection | Interval refresh |
| --- | --- | --- |
| PostgreSQL | `BEGIN READ ONLY`, `statement_timeout`, `lock_timeout` | yes |
| MySQL / MariaDB | `START TRANSACTION READ ONLY` plus `MAX_EXECUTION_TIME` / `max_statement_time` | yes |
| SQLite, libSQL file | `PRAGMA query_only=ON` in a transaction | yes |
| SQL Server | classification, `LOCK_TIMEOUT`, rolled-back transaction | manual only |
| ClickHouse, remote libSQL | classification only | manual only |
| DuckDB | unsupported (no extra session) | — |

Limits are set per widget: up to 10,000 rows (default 1,000), 1 KiB to 8 MiB of
cell text (default 1 MiB) and 1 to 60 s (default 15 s). Only the first result
set and up to 64 columns are kept, and cells are clipped at 4 KiB. A reached
limit stops the stream and is shown on the widget. The deadline drops the run,
cancels the statement and closes the connection. Interval refresh starts only
after an explicit Run. It stops on Stop, when the board is closed, switched or
edited, on disconnect, and on lock (the Monitor unmounts while locked).

Export shows a preview of the exact versioned JSON. It contains widget SQL,
which may include sensitive literals, but no results, credentials or connection
details. Import accepts a pasted or chosen file of at most 256 KiB. Unknown
versions, unknown fields (for example an embedded approval flag), malformed JSON
and rule violations are rejected. Statements must still classify as reads. An
imported board belongs to the current connection, which is its connection
remapping. It is stored unreviewed and shows every query with **Approve and
enable** until the user approves it.

Charts are bounded SVG without a chart library. Line and area series are
downsampled to 300 buckets, keeping each bucket's minimum, maximum and gaps;
NULL values are gaps. Non-numeric y columns fall back to the table. Every chart
widget has a Chart/Table toggle that shows the same values, and a unit test
checks this parity on a fixture.

Tests: core binding (bound never spliced, PostgreSQL/MySQL placeholders,
typed values, defaults, undeclared names), read-only admission, import
validation (version, oversize, malformed, unknown fields, limits) and bounded
collection (row/byte truncation, column kinds); store review state; engine
protection; and Vitest for chart/table parity, downsampling, no auto-run on
import or approval, explicit Run with bound values, and the single Add widget
action on an empty board. No live-database board test is included yet.

## 18. Implemented (M25-08): health and analytics reports

**Export report…** in the Monitor header and on a board's toolbar opens a
compact sheet with three choices: range, sections and format. The Monitor
offers the live ten-minute window and, when local history is enabled for the
profile, 1, 6 or 24 hours of stored history. Its sections are summary and
source status, metric charts, top queries and incidents. A board report holds
a summary plus the chosen widgets, taken from their latest runs. The sheet
reads the data shown when it opens and saves through the native save panel
(`report_save`, which mirrors `diagram_save`). Nothing runs against the
database: a widget that was not run is reported as having no result.

All formats render one versioned document (`openrows.monitor-report`,
version 1). The document records the title, connection name, engine,
database, range, generation time and sections. Each section is a summary,
chart, table or stat and carries its own evidence:

- **Source**: the live collector source, stored history aggregates, the
  incident rules or the board widget's query, with its read-only mode.
- **Collection range**: the report range for metrics and incidents, the live
  ranking window for top queries, and the run time for widgets.
- **Scope**: server, database, node, file, own requests or query result.
- **Gaps and unknowns**, listed explicitly. These include unknown, reset or
  denied observations (counted by state), every collection interval with no
  observation, unavailable sources, partial coverage, truncation limits,
  failed or missing runs and the session-only incident retention. Unknown
  values stay `null` in JSON, empty cells with a state in CSV, and `unknown`
  in Markdown and HTML. They are never zeros.

| Format | Content |
| --- | --- |
| Markdown | Metadata, evidence and gaps per section, tables, per-series latest/min/max/mean |
| HTML | Self-contained page: inline style and inline SVG charts, no scripts, event handlers or external resources; a CSP (`default-src 'none'`) is included |
| JSON | The versioned document, with every chart observation |
| CSV | One table section, or one chart as one row per observation (series, time, value, state) |
| SVG / PNG | One chart at 640 × 260 with fixed colours. Gaps break the line and are marked on the axis; at most 300 buckets per series keep minimum, maximum and gaps. PNG is the SVG drawn on a 2× canvas, as the ER diagram export does |

Numbers are rounded once, to six decimals, when the document is built, so
JSON, CSV, HTML and Markdown print the same values. Database-provided text
(query text, names, values) is escaped for each target. HTML and SVG escape
`& < > " '`. Markdown entity-escapes raw HTML, backslash-escapes inline syntax
and pipes, and folds line breaks. CSV quotes per RFC 4180 and prefixes text
starting with `=`, `+`, `-`, `@`, tab or carriage return with `'`, unless it
is a plain number.

Tests (Vitest, `features/monitor/report`) use hostile fixtures: `<script>`,
`<img onerror>`, formulas, pipes and newlines. The HTML is parsed and checked
for script elements, `on*` handlers and `src`/`href` attributes. The tests also
check gap preservation (unknown points, missing intervals, empty metrics,
denied sources, missing or truncated widget runs) and that one fixture prints
the same numbers in all four document formats. Further tests check that the
standalone SVG parses with gap breaks and escaped labels, that the PNG path
produces PNG bytes (the canvas is stubbed in jsdom), and that the dialog's
section, format and item choices reach the save command. The Rust test covers
the `report_save` file kinds.

## 19. Implemented (M25-07e): DuckDB diagnostics on the owning session

DuckDB is admitted to the Monitor without opening anything. The collector is a
`FacetMonitor` over the profile's one shared session (ADR-0010 M2-04b facet
port): every read is a facet call queued on the session actor after the
statements already queued, and runs on the same hosted connection and file
reservation. Monitoring starts only after an explicit DuckDB connect; Explain
and HypoPG diagnostics stay refused for DuckDB. The Overview states the
boundary: “Own OpenRows session only — other processes are not visible.”, and
shows the Overview, Incidents and Own queries panels only.

- **Coexistence**: discovery runs no query, so starting the Monitor never waits
  on a running statement. A read waits at most three quarters of its deadline
  for the session; if a user statement is still running the sample is a Gap
  (never a stall), and the queued call is skipped when the actor reaches it, so
  abandoned reads cannot pile up behind user work. A diagnostic statement that
  has started is cancelled on the host after its deadline (at most 1 s) and its
  reply drained, keeping the protocol in step. The collector never cancels the
  user's statement.
- **Database** source: one bounded statement reads `pragma_database_size()`
  (block size, total/used/free blocks and WAL size) for the current database,
  `sum(memory_usage_bytes)` from `duckdb_memory()`, `memory_limit`, the count
  and total size of `duckdb_temporary_files()` and `threads`, plus own request
  totals. Database size is block size × total blocks; formatted sizes are
  parsed to bytes and an unparsable value stays unknown. In-memory databases
  report only what DuckDB returns for them.
- **Own queries**: statements run through the editor, grid or jobs on the
  hosted session are recorded after they finish in the process-wide ledger
  from §16 (keyed by the canonical file path, or per host for in-memory
  databases); monitor and catalog reads are not. DuckDB exposes no step
  counters, so those columns stay empty.
- **Unavailable**: sessions/locks, waits/I/O, per-table and per-index
  statistics and replication carry an explicit reason, never zero.
- **Host loss**: a host failure or closed session makes the next reading a
  terminal unavailable state naming the cause (for example “The owning
  session's driver host exited …”); collection stops until an explicit
  reconnect and restart.

History keeps the reviewed scalars `database_bytes`, `block_size`,
`total_blocks`, `used_blocks`, `free_blocks`, `wal_bytes`, `memory_bytes`,
`memory_limit_bytes`, `temp_files`, `temp_bytes`, `threads` and the own request
totals. `crates/openrows-driver-host/tests/duckdb_monitor.rs` samples a temp
DuckDB file idle, during a running query (Gap within the deadline), after
cancelling it, and after an aborted host (terminal state). Own-query profiling
through `enable_profiling` and live memory during busy SQL remain follow-up.

## 20. Monitoring exit audit (M25-09)

Audit date 2026-10-12. This section checks M25-01..08 and M25-10 against §8
items 1–6 and ADR-0011. Items 7–10 belong to the M4.5 harness (M45-09). Code
was read on the audit branch, and the suites below were run on an Apple Mac mini
M4 (Mac16,10, 24 GiB, macOS 26.5, native arm64). Performance numbers and the
commands still to run are in
[ADR-0011](adr/0011-performance-monitor.md#m25-09-exit-audit--validation-matrix-results).
"Earlier evidence" means the fixture passed in that slice and was not re-run
here.

| Area | Criterion | Result | Evidence |
| --- | --- | --- | --- |
| Lifecycle | Collection starts only on an explicit Monitor open or resume. Unlock and app launch never restart it. | Pass | `security_unlock` starts nothing. `MonitorTab` sets paused on lock. Monitor tabs are not persisted (`tabStorage.ts`). |
| Lifecycle | Pause, close, disconnect, profile edit or delete, network change, lock and exit stop collectors and clear the background opt-in. | Pass | `monitor_pause` and `registry::stop_where`; `connection.rs`, `profiles.rs`, `resilient.rs`, `lib.rs` exit hook and `security` lock and idle paths. Lifecycle tests in `openrows-monitor`. |
| Lifecycle | Reads never overlap and never block the user's query. | Pass | Serial runtime and deadlines (unit tests). DuckDB busy-session gap: `duckdb_monitor.rs` (earlier evidence). |
| Lifecycle | Incidents are evaluated only for running, unlocked collectors. | Pass | `alerts.rs` lock check; evaluation only from the running collector's event loop. |
| Permissions | PostgreSQL: restricted role, revoked grant, missing extension and statistics reset are explicit states. | Pass | PG 17.11 monitor fixture, run in this audit. |
| Permissions | MySQL/MariaDB, SQL Server, ClickHouse, SQLite/libSQL and DuckDB denied or unavailable states. | Pass on tested versions | Earlier evidence: MySQL 8.4, MariaDB 11.4, Azure SQL Edge, ClickHouse 25.3, sqld v0.24.32, temp DuckDB. The wider version matrix is not measured. |
| Privacy | History stores only allowlisted scalars, with no statement text, rows or credentials. | Pass | Migration 0007 has no text column. `validate` checks names, states and finite values. Store tests (197) passed. Every engine's emitted scalar is on the allowlist. |
| Privacy | SQL is redacted in core before Arrow, IPC, rings, history and advice. | Pass | `retention_support.rs` normalises query cells before retention and Arrow encoding. Redaction tests in `openrows-monitor`. |
| Privacy | Logs never contain SQL or server messages. | Pass | The only monitor log line records the error kind of a failed history write. |
| Privacy | Report export escapes database text, uses no scripts and runs no queries. | Pass | `report/html.ts` CSP `default-src 'none'`. Reports read only shown data, history and incidents. Vitest hostile fixtures. |
| Board safety | No auto-run on open, import or approval. Unreviewed boards are refused natively, and SQL comes from the store. | Pass | `boards/run.rs` reviewed check and store load; `useBoardRuns`; Vitest no-auto-run tests. |
| Board safety | Read-only execution, limits, 256 KiB import and unknown-field rejection. | Pass | `protection.rs`, `validate.rs`, `model.rs` with `deny_unknown_fields`. A live-database board run test is not included. |
| Labels | Own-session boundary text for SQLite/libSQL file, remote libSQL and DuckDB. | Pass | `MonitorOverview.tsx` exact strings. |
| Labels | Panels match each engine's sources. | Pass | ClickHouse hides Blocking and Indexes. Embedded engines and DuckDB show Overview, Incidents and Own queries only. |
| Labels | DuckDB shows "Updated after query completion" (ADR-0011 decision 4). | Fixed | The DuckDB overview note did not use this wording; it now starts with it. |
| Labels | Explain and HypoPG are offered only where they work. | Fixed | Top queries showed **Explain** on every engine, and the native gate let MySQL, SQL Server, ClickHouse, SQLite and libSQL into the PostgreSQL-only diagnostic session. **Explain** is now shown only for PostgreSQL, other engines point to the editor's Explain, and `monitor_explain`/`monitor_hypopg` refuse non-PostgreSQL profiles (`reader::ensure_postgres`, unit-tested). |
| Performance | 1,000-row analysis p95 ≤ 10 ms, source deadline and retained-bytes caps. | Pass on M4 | ADR-0011 audit table. |
| Performance | mac-low CPU, all-process footprint, ≤ 1% server overhead, native UI timing. | Not measured | Dev-machine `pgbench` noise exceeds the 1% target. No Monitor-open footprint scenario exists yet. |
| History | 24 h retention within 50 MiB per connection. | Partial | The caps hold, but at 16 scalars every 5 s the conservative 2 KiB row charge keeps about 133 minutes. Rows are evicted oldest first instead of being downsampled. |

Open gaps, each larger than an audit fix:

1. History downsampling: coarsen old buckets (or charge measured bytes) so that
   24 h fits within 50 MiB at the default cadence. At-cap writes also take about
   23 ms under the store lock (ADR-0011 table).
2. A history batch with an unknown metric name rolls back the whole tick and
   only logs a warning. Every current engine name is allowlisted; filtering
   unknown names before the write would make this robust.
3. Monitor Explain, HypoPG and advisor rules are PostgreSQL only. Other engines
   have no advisor rules.
4. The live matrix not run here (PG 12/13/15/16/18, MySQL 5.7/8.0, MariaDB 10.6,
   SQL Server 2017–2022 and Azure SQL Database, ClickHouse multi-node and Cloud)
   and the missing PostgreSQL fixtures (managed-provider denial, unloaded
   extension, replication lag, spill/vacuum, statement reset).
5. Query Store, Always On, DuckDB `enable_profiling`, `sqlite3_db_status` and
   Hrana server timings are not implemented. The UI labels them unavailable.
6. mac-low and reference-workload certification, plus VoiceOver/NVDA and
   native WKWebView checks.

The M2.5 exit is **not met**. Lifecycle, permission states, privacy, board
safety and labels pass on the tested versions, with two label defects fixed in
this audit. The reference-hardware budgets, the history downsampling rule and
the wider engine matrix remain open.
