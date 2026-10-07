# 27 — Delivery Plan: Tasks to 1.0

The ordered task backlog from the current state to the 1.0 release. [19 — Roadmap](19-roadmap.md) defines *what* each milestone delivers and its exit criteria; this document breaks each milestone into tasks small enough for one branch and one pull request, in dependency order.

Every task follows [25 — Engineering Standards](25-engineering-standards.md): spec with acceptance criteria and test plan first, isolated worktree, acceptance report, maintainer green light, squash merge, clean-up.

## 1. How to use this plan

- **Pick the first task whose status is `todo` and whose dependencies are `done`.** Tasks within a milestone are listed in a workable order; independent tasks may run in parallel in separate worktrees.
- **Create a GitHub issue per task** titled `<ID>: <title>`, containing the full spec in the docs/25 §2 template. The rows here are the seed: goal, requirement IDs, dependencies and the gates that must appear among the acceptance criteria.
- **Requirement IDs** refer to [02 — Requirements](02-requirements.md). A task is only done when the acceptance text of each listed requirement is fully met (or the task states which part it covers and which later task covers the rest).
- **Benchmarks** refer to [13 — Performance](13-performance.md) §4. A task that touches a hot path adds or updates a benchmark and reports numbers against budget.
- **Update this file in the same PR** that completes a task (status → `done`, PR link). Add new tasks here when scope is discovered rather than growing an existing task.

Status values: `done`, `todo`, `blocked` (state the blocker), `hw` (needs specific hardware or a manual run).

### 1.1 Task ID scheme

`M<milestone>-<nn>` for feature tasks (for example `M1-09`), `M0-S<n>` for spikes, `X-<nn>` for cross-cutting tasks that are not tied to one milestone.

### 1.2 Definition of done (every task)

1. All acceptance criteria pass with evidence in the PR's acceptance report.
2. `just check` passes locally **before** pushing and merging. CI runs only on `main` after merge (docs/18 §4); if it fails, fix `main` immediately.
3. Limits from docs/25 §4 hold (functions ≤ 40 lines target / 80 hard, files ≤ 300 / 600, complexity ≤ 10 / 20, ≤ 4 params), no dead code, no TODO without an issue link.
4. Benchmarks for touched hot paths run and stay within budget; regressions are explained.
5. Docs updated in the same PR (requirements status, architecture docs, ADRs, this plan).
6. Maintainer green light after trying the change locally ("How to try it").
7. Worktree, branch (local and remote), build output and temporary files removed after merge.

## 2. Current state (2026-10-07)

| ID | Task | Status | PR |
| --- | --- | --- | --- |
| M0-01 | Engineering standards and delivery process (docs/25) | done | #1 |
| M0-02 | Rust workspace skeleton and quality gates | done | #4 |
| M0-03 | Minimal CI pipeline | done | #5 |
| M0-04 | Tauri 2 + React app shell with typed IPC (`tauri-specta`) | done | #7 |
| M0-05 | Design tokens and base components (`@openrows/ui`) | done | #9 |
| M0-06 | Hard-coded workspace mock (visual direction, navigation model) | done | #11 |
| M0-07 | Feature inventory and parity plan (docs/26) | done | #14 |
| M0-S6 | Startup and idle-memory benchmark harness (`just bench-startup`, `just bench-memory`) | done | #16 |
| M0-08 | Idle memory footprint below budget | done | #18 |
| M0-S1 | IPC throughput spike: Arrow tiles over raw responses (ADR-0002 accepted) | done | #20 |
| M0-S2 | Grid renderer spike: own Canvas 2D painter (ADR-0004 accepted) | done | #23 |

Measured on Mac mini M4 (release build): warm start p50 ≈ 290–330 ms, idle footprint ≈ 122–134 MB, 256×20 tile round trip p95 ≤ 1 ms, tile encode 11.5 µs, grid best-repeat 0 frames > 32 ms over 1 M rows.

Existing crates and packages: `openrows-types`, `openrows-results`, `openrows-app` (apps/desktop/src-tauri); `@openrows/ui`, `@openrows/ipc`, `@openrows/grid`; the workspace mock lives in `apps/desktop/src/mock/`.

## 3. M0 — Foundations and spikes (remaining)

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M0-S3 | **PostgreSQL streaming spike.** Draft `openrows-driver-api` (minimal session/stream traits) and a prototype `openrows-driver-postgres` decoding the binary protocol directly into Arrow builders; containerised PostgreSQL fixture; Criterion `decode_pg_int`, `decode_pg_text`, `decode_pg_mixed`, `decode_pg_wide`. | NFR-PERF-05, NFR-PERF-03; docs/04, docs/05, docs/06; ADR-0002, ADR-0007 | M0-S1 | ≥ 1 M cells/s/core (primitive types); first batch ≤ 5 ms; no per-cell heap allocation for primitives | done (#25) |
| M0-S4 | **SSH spike.** `openrows-net` prototype with `russh`: password, private key (ed25519/ecdsa/rsa, OpenSSH/PEM, passphrase), ssh-agent, keyboard-interactive, multi-hop jump host, `~/.ssh/config` aliases, `known_hosts` verification with first-use fingerprint. | FR-CONN-04, FR-NET-01, FR-NET-02, NFR-SEC-04; docs/24 | — | Works against an OpenSSH container matrix in tests; manual run against a real bastion recorded | todo |
| M0-S5 | **Editor spike.** CodeMirror 6 in WKWebView: 5 000-line SQL file, PostgreSQL dialect highlighting, Vim mode, IME (Japanese/Chinese input) check; spike-mode benchmark `just bench-editor` (pattern of `bench-ipc`/`bench-grid`). | NFR-PERF-07, FR-EDIT-02, FR-EDIT-09; docs/09; ADR-0003 | — | Keystroke-to-paint p95 ≤ 16 ms; editor chunk lazy, outside the initial bundle; ADR-0003 accepted with numbers | todo |
| M0-09 | **Startup follow-up.** Warm p50 rose ≈ 35 ms when panels became lazy (#18). Preload first-view chunks during idle or inline them; keep the initial bundle ≤ 350 KB. | NFR-PERF-01; docs/13 §2.1 | — | Warm p50 back near 290 ms on the M4; memory budget still met | todo |
| M0-10 | **Build cache issue.** `sccache` intermittently fails with "Operation not permitted" for `clippy-driver` after sandboxed agent runs; find the cause (server started inside a sandbox, socket/permissions) and document the fix in docs/18. | docs/18 | — | `just check` with `RUSTC_WRAPPER=sccache` works reliably | todo |
| M0-11 | **ADR review.** Accept or revise ADR-0001, 0003 (after S5), 0005, 0006, 0007 (after S3), 0008 (may stay Proposed until M4), 0009; record evidence. | docs/adr | S3, S5 | Every ADR has an explicit status and evidence | todo |
| M0-S6b | **Startup on mac-low.** Run `just bench-startup` and `just bench-memory` on the reference low-end Mac (docs/13 hardware table) and record results. | NFR-PERF-01/02 | — | Budgets met on mac-low or an optimisation task is filed | hw |

**M0 exit:** all spikes met or a decision recorded; ADRs settled; budgets met on reference hardware.

## 4. M1 — macOS alpha (PostgreSQL, MySQL/MariaDB, SQLite)

Order: core runtime → local storage and secrets → networking → drivers → result pipeline → schema → UI wiring → editing and safety → export → hardening → release.

### 4.1 Core runtime and storage

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-01 | **Session manager (`openrows-core`).** Actor-per-session runtime (ADR-0007): open/close, command queue, cancellation tokens, timeouts, typed error model, panic isolation per session, event stream to the UI (coalesced ≤ 10 Hz progress). | NFR-REL-02, NFR-REL-04, FR-EDIT-04; docs/04, docs/14 | M0-S3 | A panicking driver task does not take down the app (test); cancel request sent ≤ 200 ms after command (test) | todo |
| M1-02 | **Local store (`openrows-store`).** SQLite store with migrations for profiles, groups, history, settings, session state, change-set journal; WAL; versioned schema; backup on migration. | FR-APP-05, NFR-REL-01; docs/15 | — | Migration tests up/down; crash-safety test (kill during write) | todo |
| M1-03 | **Secrets (`openrows-secrets`).** macOS Keychain backend (Windows/Linux stubs compile per NFR-PORT-03), secret references in profiles, explicit reveal flow, zeroisation, never logged. | FR-CONN-03, NFR-SEC-01; docs/12 | M1-02 | Log-scrubbing test; secrets never cross IPC unless revealed (test) | todo |
| M1-04 | **Typed IPC surface.** Command/event contract for sessions, profiles, schema, results, settings; generated bindings; raw binary path for tiles (from S1) moved out of spike code. | NFR-MAINT-02; docs/06 | M1-01 | `just ipc-check` fresh; no hand-written duplicate types | todo |

### 4.2 Connections and networking

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-05 | **Connection profiles.** Create/edit/duplicate/delete/group/search; URL parsing for every Tier 1 scheme (fuzz target); environment tags with colours; per-connection read-only and confirm-before-write (prod default). | FR-CONN-01, 02, 06, 07; docs/24 | M1-02, M1-03 | URL parser fuzzed; prod profiles default to confirm-before-write | todo |
| M1-06 | **Network layer (`openrows-net`).** Route builder: direct, Unix socket, SOCKS5, HTTP CONNECT, multi-hop SSH (from S4), TLS via rustls with verify-full default, custom CA, client cert/key, visibly flagged skip-verify; SSH multiplexing per profile; bundled cloud CA bundles. | FR-NET-01, 02, 06, FR-CONN-04, 05, NFR-SEC-03, 04 | M0-S4 | Container matrix tests for each hop type; insecure options show a badge | todo |
| M1-07 | **Connection test diagnostics.** Step-by-step DNS → TCP → SSH → TLS → auth → version with per-hop timing, TLS certificate details, latency indicator. | FR-CONN-08, FR-NET-05 | M1-06 | Each failure step produces a specific, actionable message (tests) | todo |
| M1-08 | **Reconnect and resilience.** Keepalive probes, sleep/wake and network-change detection, idle auto-reconnect, never auto-retry in-flight writes. | FR-NET-04, NFR-REL-03 | M1-06, M1-01 | Network-loss detection within keepalive interval (test with toxiproxy or similar) | todo |
| M1-09 | **Connection UI.** Connection list/groups in the rail, new/edit connection sheet (form + URL paste), test button with diagnostics, keychain choice, environment colour in window chrome, tabs and status bar. | FR-CONN-01..08 UI; docs/20 | M1-05, M1-07 | Keyboard-only flow; axe clean; matches docs/20 visual rules | todo |

### 4.3 Drivers and result pipeline

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-10 | **Driver API and conformance suite.** Finalise `openrows-driver-api` traits (capabilities, sessions, streaming, cancellation, schema introspection, DDL generation, editing support) and `openrows-driver-conformance` test suite runnable against containers. | NFR-MAINT-01; docs/05 | M0-S3, M1-01 | Suite documents each capability; a new driver needs no UI change for baseline features | todo |
| M1-11 | **PostgreSQL driver (Tier 1).** Binary decoding into Arrow, all common types with extension metadata (JSON, UUID, numeric, arrays, ranges, geometry as unknown-with-type-name), cancel via cancel request, notices, flavour detection (CockroachDB, Timescale, Supabase, Neon, Redshift-as-later-tier), startup commands. | FR-APP-07, FR-CONN-10, NFR-PERF-05; docs/05 | M1-10 | Conformance green on supported PG versions; decode benchmarks within budget | todo |
| M1-12 | **MySQL/MariaDB driver (Tier 1).** Binary protocol into Arrow, cancel via KILL QUERY on a side connection, multiple result sets, flavour detection (MariaDB, TiDB, PlanetScale). | FR-APP-07 | M1-10 | Conformance green on MySQL 8.x/9.x, MariaDB LTS | todo |
| M1-13 | **SQLite driver (Tier 1).** Local file and in-memory, read-only open, interrupt-based cancel, attached databases, extension loading disabled by default. | FR-APP-07 | M1-10 | Conformance green | todo |
| M1-14 | **Result store production.** Promote `openrows-results`: memory accounting, spill to Arrow IPC files with LZ4 beyond budget (default 512 MB), result lifetime and eviction, first-batch fast path, window requests from the grid. | FR-GRID-01, NFR-PERF-03, NFR-PERF-06; docs/06 | M1-11 | 5 M-row stream stays within memory budget; first rows ≤ 50 ms after first batch | todo |
| M1-15 | **SQL toolkit (`openrows-sql`).** Statement splitter per dialect (PG dollar quotes, MySQL delimiters, comments), current-statement detection by cursor, safety classification (read/write/DDL/dangerous: unbounded UPDATE/DELETE, DROP, TRUNCATE), parameter placeholder detection. | FR-EDIT-03, FR-CONN-07; docs/09 | — | Fuzzed splitters; `sql_split_*` benchmarks | todo |

### 4.4 Schema

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-16 | **Schema cache (`openrows-schema`).** Lazy introspection per object kind, persisted snapshots (instant tree on reconnect), invalidation after DDL, prebuilt search index in a worker. | FR-SCHEMA-01, 02, NFR-PERF-08; docs/10 | M1-11 | 50 000 objects: first level ≤ 300 ms, search ≤ 50 ms warm | todo |
| M1-17 | **Structure and DDL views.** Columns, types, defaults, nullability, comments, indexes, constraints, foreign keys, triggers, partitions; generated DDL with copy. | FR-SCHEMA-03, 04 | M1-16 | Matches server-generated DDL in conformance fixtures | todo |

### 4.5 Workspace UI on real data

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-18 | **Workspace wiring.** Replace mock data with real sessions: rail = open workspaces (connections), sidebar = schema tree with search, database selector, ⌘K database switcher, ⌘P quick open, ⌘⇧P command palette with every action; per-database tab sets. Remove the corresponding mock code (no dead code). | FR-WS-04 (baseline), FR-APP-01, FR-SCHEMA-01/02 UI; docs/20 | M1-09, M1-16 | Journeys use real containers; mock removed where replaced | todo |
| M1-19 | **Production grid.** Integrate `@openrows/grid` into table and query tabs: selection (cell/range/row/column), column resize/reorder/hide/pin/auto-size, type-aware rendering, pagination for tables and infinite mode for queries, client sort/filter for ad-hoc results, server filter/sort for tables (filter bar), copy as TSV/CSV/JSON/Markdown/SQL INSERT/`WHERE IN`; shadow ARIA grid production-ready. Visual check against the reference app (density, greys). | FR-GRID-01..05, 08, NFR-PERF-04, NFR-UX-02; docs/08 | M1-14, M1-18 | `just bench-grid` passes; VoiceOver reads active cell; copy formats tested | todo |
| M1-20 | **Inspector panel.** Cell inspector: full value, JSON tree, hex view, image preview; row details. | FR-GRID-06 | M1-19 | Large values streamed, not loaded into the grid | todo |
| M1-21 | **SQL editor production.** CodeMirror 6 (from S5): dialect highlighting, schema-aware alias-aware autocomplete, run all / selection / current statement, multiple result tabs, cancel, tabs per connection restored on relaunch with unsaved text persisted within 1 s. | FR-EDIT-01..04, NFR-PERF-07, NFR-REL-01; docs/09 | M0-S5, M1-15, M1-16 | Journeys 2 and 3 pass; autocomplete popup ≤ 50 ms | todo |
| M1-22 | **History.** Every executed statement recorded (connection, duration, rows, status), full-text search, per-connection disable, History sidebar tab. | FR-WS-01, 02 | M1-21 | Disabled connections record nothing (test) | todo |

### 4.6 Editing, safety and export

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-23 | **Data editing.** Inline edit, insert, duplicate, delete when the result maps to one table with a primary/unique non-null key; staged change set with highlights; undo/redo; Review & Commit with generated SQL preview; single transaction with full rollback on error; change set journaled for crash recovery. | FR-DATA-01, 02, 03, 05, NFR-REL-01, NFR-UX-04; docs/10 | M1-19 | Journeys 1, 5 and 6 pass | todo |
| M1-24 | **Safe mode.** Read-only blocks writes, confirm-before-write, dangerous statement detection with explicit confirmation, prod defaults, visible state in the toolbar. | FR-CONN-07, NFR-UX-04 | M1-15, M1-23 | Journey 8 passes; no bypass via editor, grid or palette (tests) | todo |
| M1-25 | **Export.** Streamed CSV, JSON, SQL INSERT export of results and whole tables without loading rows into the UI; progress and cancel. | FR-IO-01 (P0 subset), NFR-PERF-09 | M1-14 | Journey 4 passes (checksum); ≥ 80 % of server send rate | todo |

### 4.7 App shell, quality and release

| ID | Task | Requirements / docs | Depends on | Key gates | Status |
| --- | --- | --- | --- | --- | --- |
| M1-26 | **Settings, themes and session restore.** Human-readable JSON settings with UI editor; light/dark/system and high contrast; font and density; rebindable shortcuts; session restore. | FR-APP-01, 02, 05, NFR-UX-01, 03 | M1-02 | Contrast report 0 below budget; reduced motion honoured | todo |
| M1-27 | **Native menus and tabs baseline.** macOS menu bar with every primary action, tab basics (preview/pinned tabs, reorder, close others). | FR-APP-14, FR-APP-10 (baseline) | M1-18 | Menu actions route through the command registry | todo |
| M1-28 | **Observability and crash recovery.** `tracing` with near-zero disabled cost, log rotation without secrets or query values, crash detection and recovery prompt, opt-in scrubbed crash reports, dev-build long-task monitor. | NFR-REL-05, NFR-SEC-07, NFR-PERF-10; docs/14 | M1-01 | Scrubbing tests; no telemetry by default | todo |
| M1-29 | **End-to-end tests.** macOS smoke suite through the dev-only test-automation IPC; Linux CI E2E with `tauri-driver` and containerised databases covering critical journeys 1–6 and 8 (docs/17). | docs/17 | M1-23..25 | Journeys automated and green in CI | todo |
| M1-30 | **CI expansion.** Tier 1 conformance against containers, relative benchmark comparison on PRs, nightly fuzzing with persisted corpus, coverage ≥ 80 % for core crates. | NFR-MAINT-03, NFR-SEC-06; docs/18 §4.2 | M1-10 | CI stays under a reasonable time budget; flaky tests fixed, not retried | todo |
| M1-31 | **Release engineering (macOS).** Developer ID signing, notarisation, DMG, Homebrew cask, signed auto-update with release notes and opt-out, installer ≤ 40 MB. | FR-APP-04, NFR-SEC-05, NFR-PORT-02; docs/16 | — | Fresh-machine install and update tested | todo |
| M1-32 | **Alpha exit audit.** Requirement → test traceability for every P0; budgets on mac-low; docs/02 status columns updated. | docs/19 M1 exit | all M1 | All P0 met; journeys 1–6 and 8 green | todo |

## 5. M2 — SQL breadth and power features

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M2-01 | Driver host process (out-of-process drivers, crash isolation, IPC protocol) | docs/05, NFR-REL-02 | M1-10 |
| M2-02 | SQL Server driver (Tier 2) | FR-APP-07 | M1-10 |
| M2-03 | ClickHouse driver with native Arrow pass-through (Tier 2) | FR-APP-07 | M1-10 |
| M2-04 | DuckDB driver in the driver host (Tier 2) | FR-APP-07 | M2-01 |
| M2-05 | libSQL/Turso driver (Tier 2) | FR-APP-07 | M1-13 |
| M2-06 | Explain / explain analyze visualiser with hotspots (PG, MySQL, SQLite, MSSQL) | FR-EDIT-07 | M1-21 |
| M2-07 | Structure editing forms with DDL preview and warnings | FR-SCHEMA-05 | M1-17 |
| M2-08 | Table, trigger/view/routine and database/schema lifecycle editors | FR-SCHEMA-10, 11, 12 | M2-07 |
| M2-09 | Views, materialized views, routines, sequences, types, extensions, users/roles browsing | FR-SCHEMA-07, FR-ADMIN-02 (viewer) | M1-16 |
| M2-10 | ER diagram (read-only, auto-layout, SVG/PNG export) | FR-SCHEMA-06 | M1-16 |
| M2-11 | Import CSV/JSON/NDJSON with mapping, type preview, bulk-load fast paths, table creation and conflict preview | FR-IO-02, FR-IO-06 | M1-25 |
| M2-12 | Run `.sql` file with progress and stop-on-error | FR-IO-03 | M1-21 |
| M2-13 | Excel, Parquet, Markdown, NDJSON, TSV export; scoped export options and background jobs | FR-IO-01 (rest), FR-IO-05 | M1-25 |
| M2-14 | Dump/restore wrappers for installed client tools with version detection | FR-IO-04 | — |
| M2-15 | Process list with kill/cancel | FR-ADMIN-01 | M1-11 |
| M2-16 | SQL formatter, parameters bar, snippets | FR-EDIT-05, 06 | M1-21 |
| M2-17 | Projects (file-backed saved queries, `.openrows/project.yaml`), saved-query folders and actions | FR-WS-03, FR-EDIT-08, FR-EDIT-14 | M1-21 |
| M2-18 | Editor parity: splits, multi-caret/folding/display, completion settings, execution defaults and notices | FR-EDIT-11, 12, 13, 15, FR-EDIT-09 | M1-21 |
| M2-19 | Grid parity: filter builder with raw predicates, persistent table presentation, copy options, bulk row inspector, FK navigation, row form view, aggregate footer, explicit discard/reload | FR-GRID-07, 09..14, FR-DATA-04, 06, 08 | M1-19, M1-23 |
| M2-20 | Workspace parity: multiple contexts per window, session descriptors, history actions and retention | FR-WS-04, FR-WS-05 | M1-18, M1-22 |
| M2-21 | Connection parity: open export/import format, credential prompt policy and trusted credential commands, imports from pgpass/my.cnf/pg_service | FR-CONN-09, 12, 13 | M1-05 |
| M2-22 | App parity: settings groups and locations, tab/layout controls, safety policies and locks with OS authentication, general controls, theme JSON import/export and fonts, file associations | FR-APP-08, 10..14 | M1-26, M1-27 |
| M2-23 | Sidebar favourites, recents, visibility filters | FR-SCHEMA-09 | M1-18 |
| M2-24 | Cloud/zero-trust hops: AWS SSM, kubectl port-forward, Cloud SQL proxy, cloudflared, custom command | FR-NET-03 | M1-06 |
| M2-25 | Tier 2 conformance in CI and M2 parity exit audit | docs/19 M2 exit | all M2 |

## 6. M3 — NoSQL

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M3-01 | Document/key-value result model extensions (Arrow struct/union mapping, tree/JSON/table views) | docs/05, docs/06 | M1-14 |
| M3-02 | MongoDB driver: collections browser, MQL editor with shell-like syntax (parser fuzzed), indexes | FR-EDIT-10, FR-APP-07 | M3-01 |
| M3-03 | MongoDB document editing with JSON/EJSON validation | FR-DATA-07 | M3-02 |
| M3-04 | MongoDB aggregation pipeline editor with stage previews | docs/19 M3 | M3-02 |
| M3-05 | Redis driver: SCAN key browser, type-aware viewers/editors, TTL | FR-DATA-07, FR-APP-07 | M3-01 |
| M3-06 | Redis console with command autocomplete (tokenizer fuzzed), cluster/sentinel, pub/sub monitor | FR-EDIT-10 | M3-05 |
| M3-07 | Tier 2 NoSQL conformance and M3 exit audit | docs/19 | all M3 |

## 7. M4 — API client v1

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M4-01 | `openrows-http` engine: HTTP/1.1 + HTTP/2, all body types, cookies, proxies, client certs, timing waterfall, large bodies streamed to disk | FR-API-01, 02; docs/11 | M1-06 |
| M4-02 | Collection format (`openrows-collection`): collections and folders as files, environments, variable scopes, secrets in keychain (ADR-0006) | FR-API-04 | M1-03 |
| M4-03 | Request editor and response viewer UI (pretty/raw/preview, headers, cookies, timing) | FR-API-01, 02 | M4-01 |
| M4-04 | Auth helpers: Basic, Bearer, API key, Digest, OAuth 2.0 (auth code + PKCE, client credentials, device code), AWS SigV4 | FR-API-03 | M4-01 |
| M4-05 | Script sandbox (`openrows-script`, QuickJS, ADR-0008): pre-request/post-response scripts, assertions, `pm.*` shim, restricted mode for untrusted collections | FR-API-05 | M4-02 |
| M4-06 | GraphQL with introspection, autocomplete and docs explorer | FR-API-06 | M4-03 |
| M4-07 | Importers: Postman v2.1, Insomnia v4, Bruno, OpenAPI 3.0/3.1, HAR, cURL (all fuzzed) | FR-API-09 | M4-02 |
| M4-08 | Code generation: cURL, fetch, Python requests, Go and more | FR-API-11 | M4-03 |
| M4-09 | M4 exit: journey 7 (import Postman → run with environment → tests pass) | docs/17 | all M4 |

## 8. M4.5 — Native AI and MCP

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M45-01 | `openrows-ai` provider layer: API-key providers (Anthropic, OpenAI, Gemini, OpenAI-compatible) with keys in keychain; local models (Ollama, LM Studio) | FR-AI-01, 02; docs/23; ADR-0009 | M1-03 |
| M45-02 | Data-sharing policy: per-connection level, redaction rules, provider allowlist, context preview, audit log; golden-payload tests | FR-AI-08 | M45-01 |
| M45-03 | `openrows-mcp` server: read-only DB tools and `propose_sql`; optional localhost endpoint; scoped read-only tokens, revocation, client disconnect, scrubbed activity | FR-AI-06, FR-AI-10 | M45-02 |
| M45-04 | Agent CLI bridges: detect and drive Claude Code, Codex CLI, Cursor CLI, Gemini CLI headlessly, sandboxed to the app's MCP tools | FR-AI-03 | M45-03 |
| M45-05 | Assistant panel: NL → query, explain, fix error, optimise from plan, visible tool-call trace | FR-AI-04 | M45-01, M45-03 |
| M45-06 | Inline ⌘I prompt with accept/reject diff in the editor | FR-AI-05 | M45-05 |
| M45-07 | MCP client for user servers with per-tool enable/disable and first-use confirmation | FR-AI-07 | M45-03 |
| M45-08 | Write-safety enforcement: model output never executes writes; write proposals only open in an editor tab (tests) | FR-AI-09 | M45-05 |
| M45-09 | M4.5 exit: golden payloads green, no write path, NL→SQL eval baseline per provider | docs/19 | all M4.5 |

## 9. M5 — Long tail

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M5-01 | Oracle driver in the driver host with user-installed Instant Client (Tier 3) | FR-APP-07, NFR-PORT-02 | M2-01 |
| M5-02 | Cassandra/ScyllaDB driver with CQL editor (Tier 3) | FR-EDIT-10 | M1-10 |
| M5-03 | DynamoDB driver with PartiQL (Tier 3) | FR-EDIT-10 | M1-10 |
| M5-04 | Elasticsearch/OpenSearch driver with JSON DSL (Tier 3) | FR-EDIT-10 | M1-10 |
| M5-05 | Redshift flavour with capability limits shown (Tier 3) | FR-APP-07 | M1-11 |
| M5-06 | WebSocket and SSE with message log, composer, filters | FR-API-07 | M4-01 |
| M5-07 | gRPC unary and streaming via reflection or `.proto` | FR-API-08 | M4-01 |
| M5-08 | Collection runner and `openrows` CLI with JUnit/JSON/HTML reports | FR-API-10 | M4-05 |
| M5-09 | URL handler and CLI open command for profiles, files, object/filter links (never auto-executes SQL) | FR-APP-09 | M1-18 |
| M5-10 | DB ↔ API integration features (docs/11) | docs/19 M5 | M4, M1 |
| M5-11 | Multiple windows; move tabs between windows | FR-APP-03 | M1-27 |
| M5-12 | Localisation-ready strings | FR-APP-06 | — |
| M5-13 | Cloud auth helpers (RDS IAM, Azure AD, Cloud SQL IAM) | FR-CONN-11 (P2, optional) | M1-06 |

## 10. M6 — Windows and Linux beta

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| M6-01 | Platform layer (`openrows-platform`): Windows Credential Manager and Secret Service backends, native menus, file associations, OS authentication for locks | NFR-PORT-01, FR-APP-14, FR-APP-11 | M1-03 |
| M6-02 | WebView2 and WebKitGTK workarounds; re-run all spike benchmarks (startup, memory, IPC, grid, editor) per OS | NFR-PERF-*; docs/16 | — |
| M6-03 | Full E2E suite on Windows and Linux CI | docs/17 | M1-29 |
| M6-04 | Packaging: MSI/NSIS, winget, Scoop, AppImage, deb, rpm, Flathub; signed updates | NFR-SEC-05, NFR-PORT-02 | M1-31 |
| M6-05 | Per-OS driver capability matrix, settings, locks and multi-window layout verification | FR-APP-07, FR-APP-14 | M6-01 |

## 11. 1.0 GA

| ID | Task | Requirements | Depends on |
| --- | --- | --- | --- |
| GA-01 | Requirement traceability: every P0/P1 and every 1.0 row in docs/26 has implemented evidence | docs/19 GA, docs/26 | all |
| GA-02 | Accessibility audit: WCAG 2.2 AA chrome; grid with VoiceOver and NVDA | NFR-UX-01..03 | — |
| GA-03 | Beta programme: crash-free sessions ≥ 99.8 % (opt-in reporting); zero open data-loss/security-high issues | NFR-REL-05 | M1-28 |
| GA-04 | Documentation site: user guide, driver notes, collection format spec, keyboard reference | docs/19 GA | — |
| GA-05 | Security review: threat model re-check, dependency audit, signing/update chain review | docs/12 | — |
| GA-06 | Release 1.0 on all three platforms | docs/16 | GA-01..05 |

## 12. Cross-cutting tasks

| ID | Task | When |
| --- | --- | --- |
| X-01 | **Going public.** Restore the full CI pipeline from docs/18 §4.2 (OS matrix, supply-chain checks, cargo-deny advisories, licence checks), enable branch protection with required checks, review governance docs, secrets scan of history | Before the repository becomes public |
| X-02 | Keep benchmark baselines current: record M4 and mac-low numbers in docs/13 after each performance-relevant milestone | Each milestone exit |
| X-03 | Rename leftover "OpenDB" labels in early prototype screens to OpenRows | Any time (small) |
| X-04 | GitHub Actions runner image migration (ubuntu-latest moves to Ubuntu 26 from 2026-10-19); pin or verify | October 2026 |
| X-05 | Post-1.0 candidates from docs/19 (ADBC, plugins, schema diff, editable diagrams, team features) — plan only after GA | After GA |
