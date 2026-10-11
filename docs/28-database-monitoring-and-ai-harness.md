# 28 — Database Monitoring, Analytics and the AI Harness

Status: PostgreSQL monitoring backend, workspace UI and opt-in local history implemented (M25-01/02/03); the remaining engine, analytics and validation matrix is planned. Scope expanded on 2026-10-11. Delivery order:
finish the current DuckDB host repair, build monitoring and analytics (M2.5),
then build the database AI harness (M4.5). This document owns the expanded
FR-ADMIN-03..08 and FR-AI-11..13 scope; [27](27-delivery-plan.md) owns task status. [ADR-0011](adr/0011-performance-monitor.md) is the accepted architecture decision (owner, 2026-10-11), with implementation validation pending. It owns detailed engine sources/grants, counter semantics, budgets, privacy and advisor rules; this document owns product scope and acceptance.

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
files must retain their single owning host/session under ADR-0010: queue safe
reads through that owner once available, or expose Unsupported. Never create a
hidden second writer or metadata host to implement monitoring.

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

This is the intended product coverage summary, not a shipped capability claim.
[ADR-0011](adr/0011-performance-monitor.md#data-sources-and-permissions) owns
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
| DuckDB             | App/host query health and supported engine diagnostics through the owning session after query completion.                                             | Single-writer reservation, unavailable shared-session diagnostics until M2-04b, no server process list. |

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
sharing, routing allowlists and context preview are implemented in M45-02 (§7 below), and the harness
below is still planned. There is no silent alternate-provider/API-billing fallback.

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
and max-since-reset stay separately labelled. Full-window rankings belong to
M25-04. Statement coverage includes collector/guard commands where their query
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
unlocking requires explicit resume. Unsupported engines show a coming state.
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
Full-window rankings and deterministic advice remain M25-04.

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
reference hardware budgets remain for M25-09. The first PG run failed in an
unrelated custom-hop test and passed on an unmodified rerun.

Vitest covers source and engine capability states, gaps/reset validity,
interval/pause/resume, close/disconnect/lock and late admission, review-only
Explain, accessible chart tables, default-off history, settings persistence and
purge. Rust tests cover profile/database isolation, startup/age expiry,
retention caps, raw-text rejection and gap-preserving bucket limits. No new
dependency or chart library was required; SVG avoids a large plotting bundle.
The lazy production build reports 305,167 gzip bytes of initial JavaScript
(budget 350,000) and 6,306 gzip bytes of shared UI CSS (budget 15,000).
