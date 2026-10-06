# 02 — Requirements

IDs are stable so issues, tests and PRs can reference them (`FR-CONN-03`, `NFR-PERF-02`). Priority: **P0** = required for first public alpha, **P1** = required for 1.0, **P2** = post-1.0.

## 1. Functional requirements

### 1.1 Connections (`FR-CONN`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-CONN-01 | Create, edit, duplicate, delete, group and search connection profiles. | P0 |
| FR-CONN-02 | Parse and accept connection URLs (`postgres://`, `mysql://`, `mongodb+srv://`, `redis://`, `sqlite:` …) as well as form fields. | P0 |
| FR-CONN-03 | Store passwords, private-key passphrases and tokens in the OS keychain; profiles on disk contain only references. | P0 |
| FR-CONN-04 | SSH tunnelling with password, private key (OpenSSH/PEM, ed25519/ecdsa/rsa), and ssh-agent auth; multi-hop jump hosts; honour `~/.ssh/config` host aliases. | P0 |
| FR-CONN-05 | TLS with verify-full by default where supported; custom CA, client cert/key, and an explicit, visibly-flagged "skip verification" option. | P0 |
| FR-CONN-06 | Environment tag (local/dev/staging/prod/custom) with colour, shown in window chrome, tabs and status bar. | P0 |
| FR-CONN-07 | Per-connection read-only mode and "confirm before write" mode. Prod-tagged connections default to confirm-before-write. | P0 |
| FR-CONN-08 | Test connection with step-by-step diagnostics (DNS, TCP, SSH, TLS, auth, version). | P0 |
| FR-CONN-09 | Import connections from common tools' export formats and from `pgpass`/`.my.cnf`/`~/.pg_service.conf` where available. | P1 |
| FR-CONN-10 | Startup commands per connection (e.g. `SET search_path`, `SET ROLE`). | P1 |
| FR-CONN-11 | Cloud auth helpers: AWS RDS IAM tokens, Azure AD for SQL Server, GCP Cloud SQL IAM. | P2 |
| FR-CONN-12 | Export selected profiles, groups or all profiles in a documented, versioned open format; preserve tags and safety settings, exclude secrets and private-key contents, and validate imports with a conflict preview. Support only publicly specified third-party formats. | P1 |
| FR-CONN-13 | Choose keychain storage, prompt on each connection, or no stored password; support an explicitly trusted credential command with timeout, cancellation and scrubbed diagnostics. Never run commands from an untrusted imported profile. | P1 |

### 1.2 Schema browsing (`FR-SCHEMA`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-SCHEMA-01 | Lazy tree of databases → schemas → object kinds → objects, loaded on expand. | P0 |
| FR-SCHEMA-02 | Fuzzy search across all objects of a connection, results under 50 ms from cache. | P0 |
| FR-SCHEMA-03 | Structure view: columns, types, defaults, nullability, comments, indexes, constraints, foreign keys, triggers, partitions. | P0 |
| FR-SCHEMA-04 | Show generated DDL for any object. | P0 |
| FR-SCHEMA-05 | Edit structure through forms that generate reviewable DDL (add/alter/drop column, index, constraint). | P1 |
| FR-SCHEMA-06 | ER diagram of a schema or selection, auto-layout, export SVG/PNG. | P1 |
| FR-SCHEMA-07 | Views, materialized views, functions/procedures, sequences, enums/types, extensions, users/roles. | P1 |
| FR-SCHEMA-08 | Schema diff between two connections/schemas producing a migration script. | P2 |
| FR-SCHEMA-09 | Sidebar sections for objects, saved queries and history; persistent object favourites, recent objects, kind/schema filters, system-object visibility and visible connection/database context. | P1 |
| FR-SCHEMA-10 | Create, rename, truncate and drop tables through capability-aware forms with DDL preview and impact confirmation; unsupported operations explain their limitation. | P1 |
| FR-SCHEMA-11 | Create, edit and drop triggers, views and routines through definition editors with dialect validation, DDL preview and normal safety checks where the driver supports them. | P1 |
| FR-SCHEMA-12 | Create, rename and drop databases/schemas where supported, including encoding/collation choices; preview the statements and confirm destructive impact. | P1 |

### 1.3 Query editor (`FR-EDIT`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-EDIT-01 | Multiple editor tabs per connection, restored on relaunch including unsaved text. | P0 |
| FR-EDIT-02 | Syntax highlighting per dialect; autocomplete for keywords, schemas, tables, columns (alias-aware), functions. | P0 |
| FR-EDIT-03 | Run all, run selection, run current statement (cursor-based detection); multiple result tabs for multiple statements. | P0 |
| FR-EDIT-04 | Cancel running query from UI within 200 ms of click (cancel request sent). | P0 |
| FR-EDIT-05 | Format SQL (configurable style). | P1 |
| FR-EDIT-06 | Named/positional parameters prompt (`:name`, `$1`, `?`). | P1 |
| FR-EDIT-07 | Explain / explain analyze with visual plan tree and hotspots. | P1 |
| FR-EDIT-08 | Snippets and saved queries stored as `.sql` files in a Project folder. | P1 |
| FR-EDIT-09 | Vim keybindings option. | P1 |
| FR-EDIT-10 | Native query languages: MongoDB shell-like syntax, Redis commands, CQL, PartiQL, Elasticsearch JSON DSL. | P1 |
| FR-EDIT-11 | Independent editor/result panes split horizontally or vertically; resize, close and focus panes by keyboard while preserving each pane's connection context. | P1 |
| FR-EDIT-12 | Multiple carets, rectangular selection, occurrence selection, line/block comments, folding, visible whitespace, wrapping and configurable indentation; retain the editor latency budget. | P1 |
| FR-EDIT-13 | Completion controls for suggestion kinds, schema qualification, keyword case, closing delimiters and acceptance key; allow disabling completion without disabling highlighting. | P1 |
| FR-EDIT-14 | Named saved queries in folders with keyword expansion, insert/open/copy actions, rename/delete and reveal in the file manager; configurable file autosave separate from crash recovery, with explicit execution through safe mode. | P1 |
| FR-EDIT-15 | Configurable current-statement highlight, server-notice display, run-button default and result presentation (tabs or stacked); scripts stop on error by default, with explicit continue-on-error and per-statement status. | P1 |

### 1.4 Results and data grid (`FR-GRID`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-GRID-01 | Render result sets of any size with smooth scrolling; memory stays within budget via spill-to-disk. | P0 |
| FR-GRID-02 | Column resize, reorder, hide, pin; auto-size to content. | P0 |
| FR-GRID-03 | Cell, range, row, column selection; copy as TSV/CSV/JSON/Markdown/SQL INSERT/`WHERE IN`. | P0 |
| FR-GRID-04 | Server-side filter and sort for table views (generates `WHERE`/`ORDER BY`); client-side sort/filter for ad-hoc results. | P0 |
| FR-GRID-05 | Type-aware cell rendering: NULL, booleans, numbers right-aligned, dates, JSON, binary (hex/size), long text truncated. | P0 |
| FR-GRID-06 | Cell inspector panel: full value, JSON tree, hex view, image preview for image blobs. | P0 |
| FR-GRID-07 | Foreign-key navigation: jump to referenced row. | P1 |
| FR-GRID-08 | Pagination mode for table browsing (page size configurable) and infinite mode for queries. | P0 |
| FR-GRID-09 | Row detail / form view. | P1 |
| FR-GRID-10 | Aggregate footer for selection (count, sum, avg, min, max). | P1 |
| FR-GRID-11 | Filter builder with multiple AND/OR conditions, dialect-aware operators, any-column search, cell/header quick filters, raw SQL predicates, clear action and generated SELECT preview. Raw predicates use normal query safety checks. | P1 |
| FR-GRID-12 | Persist per-table column order/width/visibility, filter state and default sort; table settings control row striping, scrollbars and estimated counts with an explicit cancellable exact-count action. | P1 |
| FR-GRID-13 | Copy single cells, rows and columns with optional CSV headers and HTML tables; configurable delimiter, quote, newline and decimal conventions with ambiguous combinations rejected and NULL handling explicit. | P1 |
| FR-GRID-14 | Row inspector searches fields and stages bulk changes to selected rows; show affected-row count and require the ordinary change-set preview before commit. | P1 |

### 1.5 Data editing (`FR-DATA`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-DATA-01 | Inline edit cells, insert rows, duplicate rows, delete rows when the result maps to a single table with a primary key or unique non-null key. | P0 |
| FR-DATA-02 | All edits are staged in a change set; pending cells are highlighted; nothing is written until Commit. | P0 |
| FR-DATA-03 | Commit shows generated SQL preview; executes in a single transaction where supported; rolls back fully on error. | P0 |
| FR-DATA-04 | Optimistic concurrency: detect rows changed or deleted by others since load and report conflicts. | P1 |
| FR-DATA-05 | Undo/redo within the pending change set. | P0 |
| FR-DATA-06 | Type-specific editors: date/time picker, enum dropdown, JSON editor, boolean toggle, set NULL / DEFAULT. | P1 |
| FR-DATA-07 | Document editing for MongoDB (JSON/EJSON with validation); key editing for Redis by type. | P1 |
| FR-DATA-08 | Discard pending data/structure edits after impact confirmation and reload server state; clearly distinguish discard from rollback of already executed SQL, preserving the pre-discard state until confirmation. | P1 |

### 1.6 Import / export (`FR-IO`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-IO-01 | Export result or whole table to CSV, TSV, JSON, NDJSON, SQL INSERT, Markdown, Excel (`.xlsx`), Parquet. Streamed, does not load all rows into UI. | P0 (CSV/JSON/SQL), P1 others |
| FR-IO-02 | Import CSV/JSON/NDJSON into a table with column mapping, type preview, batch size, and bulk-load fast paths (e.g. PostgreSQL `COPY`). | P1 |
| FR-IO-03 | Run `.sql` file against a connection with progress and stop-on-error option. | P1 |
| FR-IO-04 | Dump/restore wrappers that invoke the user's installed `pg_dump`/`pg_restore`/`mysqldump`/`mongodump` with version detection. | P1 |
| FR-IO-05 | Export selected tables/views, query results, current page or filtered rows/columns with explicit scope; configurable CSV encoding/NULL handling and SQL structure/data/drop options; background progress, cancellation and reveal-output action. | P1 |
| FR-IO-06 | CSV/JSON import can create a table from reviewed type inference or text columns; preview mapping, skipped columns, file encoding and supported conflict policies before applying batches, with actionable error counts. | P1 |

### 1.7 Administration (`FR-ADMIN`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-ADMIN-01 | Process/session list with kill/cancel. | P1 |
| FR-ADMIN-02 | Users/roles and grants viewer; editor in P2. | P1 |
| FR-ADMIN-03 | Server metrics overview (connections, cache hit ratio, replication lag where exposed). | P2 |

<a id="18-history-and-workspace-fr-ws"></a>

### 1.8 History and Projects (`FR-WS`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-WS-01 | Every executed statement recorded with connection, duration, row count, status; full-text search. | P0 |
| FR-WS-02 | History can be disabled per connection (e.g. for sensitive prod). | P0 |
| FR-WS-03 | Projects: a folder containing saved queries, API collections and shared (non-secret) connection definitions, suitable for git. | P1 |
| FR-WS-04 | Multiple database/connection contexts per window, each with its own tabs; quick database/connection switch and quick-open for objects and saved queries, reopen session descriptors on demand without silently reconnecting or executing SQL. | P1 |
| FR-WS-05 | History actions to insert, copy, open and save a statement as a query; delete entries, clear history and configure retention, with sensitive literals redacted according to the connection's policy. | P1 |

### 1.9 API client (`FR-API`)

Detailed in [11-api-client.md](11-api-client.md).

| ID | Requirement | P |
| --- | --- | --- |
| FR-API-01 | HTTP requests: all methods, query/path params, headers, bodies (JSON, text, XML, form-urlencoded, multipart, binary file, GraphQL). | P1 |
| FR-API-02 | Response viewer: pretty/raw/preview, headers, cookies, timing breakdown, size; large bodies streamed to disk. | P1 |
| FR-API-03 | Auth helpers: Basic, Bearer, API key, Digest, OAuth 2.0 (auth code + PKCE, client credentials, device code), AWS SigV4. | P1 |
| FR-API-04 | Collections and folders as files; environments with variables; secrets in keychain. | P1 |
| FR-API-05 | Pre-request and post-response scripts, assertions; sandboxed JS. | P1 |
| FR-API-06 | GraphQL with introspection-driven autocomplete and docs explorer. | P1 |
| FR-API-07 | WebSocket and SSE with message log, send composer, filters. | P1 |
| FR-API-08 | gRPC unary and streaming via server reflection or imported `.proto`. | P1 |
| FR-API-09 | Importers: Postman v2.1, Insomnia v4, Bruno, OpenAPI 3.0/3.1, HAR, cURL. | P1 |
| FR-API-10 | Collection runner (in-app and `openrows` CLI) with JUnit/JSON reports. | P1 |
| FR-API-11 | Code generation: cURL, fetch, Python requests, Go, and more. | P1 |

### 1.10 AI assistant (`FR-AI`)

Detailed in [23-ai-assistant.md](23-ai-assistant.md).

| ID | Requirement | P |
| --- | --- | --- |
| FR-AI-01 | Configure API-key providers (native + OpenAI-compatible) with keys in the keychain. | P1 |
| FR-AI-02 | Use local models (Ollama, LM Studio, OpenAI-compatible localhost servers). | P1 |
| FR-AI-03 | Detect and drive installed agent CLIs (Claude Code, Codex CLI, Cursor CLI, Gemini CLI) headlessly, sandboxed to app MCP tools. | P1 |
| FR-AI-04 | Assistant panel: NL → query, explain, fix error, optimise from plan, with visible tool-call trace. | P1 |
| FR-AI-05 | Inline ⌘I prompt in the editor producing an accept/reject diff. | P1 |
| FR-AI-06 | Built-in MCP server exposing read-only DB tools and `propose_sql`; optional localhost HTTP endpoint for external agents. | P1 |
| FR-AI-07 | MCP client for user-configured servers, per-tool enable/disable and first-use confirmation. | P1 |
| FR-AI-08 | Per-connection data-sharing level, redaction rules, provider allowlist, context preview, audit log. | P1 |
| FR-AI-09 | Model output can never execute writes; write-classified proposals only open in an editor tab. | P1 |
| FR-AI-10 | MCP settings show server status, transport and exposed connections; create and revoke scoped read-only access tokens, disconnect clients and inspect scrubbed request activity. Revocation cancels active requests; token values never appear in settings JSON. | P1 |

### 1.11 Networking (`FR-NET`)

Detailed in [24-networking-and-protocols.md](24-networking-and-protocols.md).

| ID | Requirement | P |
| --- | --- | --- |
| FR-NET-01 | Route builder: direct, Unix socket, SOCKS5, HTTP CONNECT, multi-hop SSH, TLS. | P0 |
| FR-NET-02 | SSH session multiplexing across all sessions of a profile; MFA prompts; `~/.ssh/config` support. | P0 |
| FR-NET-03 | Cloud/zero-trust hops: AWS SSM, kubectl port-forward, Cloud SQL/AlloyDB proxy, cloudflared, custom command. | P1 |
| FR-NET-04 | Proactive probe and reconnect on sleep/wake and network change. | P0 |
| FR-NET-05 | Per-hop diagnostics, route display, TLS certificate details, latency indicator. | P0 |
| FR-NET-06 | Bundled updatable cloud CA bundles so `verify-full` works for RDS/Azure/GCP. | P1 |

### 1.12 App-wide (`FR-APP`)

| ID | Requirement | P |
| --- | --- | --- |
| FR-APP-01 | Command palette with every action; fully rebindable shortcuts. | P0 |
| FR-APP-02 | Light/dark/system themes; high-contrast option; font and density settings. | P0 |
| FR-APP-03 | Multiple windows; tabs can move between windows. | P1 |
| FR-APP-04 | Signed auto-update with release notes; user can disable. | P0 |
| FR-APP-05 | Settings stored as human-readable JSON with a UI editor. | P0 |
| FR-APP-06 | Localisation-ready strings (English first). | P1 |
| FR-APP-07 | Database support follows the versioned driver/capability matrix in docs/05: Tier 1 in M1, Tier 2 in M2/M3, Tier 3 including Redshift in M5; display unsupported features and beta status instead of implying identical engine support. | P1 |
| FR-APP-08 | Searchable settings groups for general behaviour, tables, editor, appearance, security, locations, keymap, assistant and MCP; validated location changes, reset defaults and portable non-secret settings export. Never relocate secrets from the keychain or silently sync history. | P1 |
| FR-APP-09 | Documented OpenRows URL handler and CLI open command for profiles, local database files and object/filter links; percent-encoding validation and context preview. Links never auto-run SQL or weaken connection safety and omit secrets on copy. | P1 |
| FR-APP-10 | Object preview/pinned tabs, rename/reorder, close-other/right tabs and move between windows; keyboard pane/tab/workspace navigation, session-layout restore and dirty-state confirmation. | P1 |
| FR-APP-11 | Explicit safety policies: unrestricted, confirm all, confirm writes, authenticate all, authenticate writes and read-only; optional OS authentication and startup/inactivity app lock. Context switches cannot bypass locks; prod defaults remain confirm-before-write. | P1 |
| FR-APP-12 | General settings select stable/beta updates, update notifications, session restore and new-tab behaviour; query timeout/keep-alive controls and opt-in crash reporting stay visible and free to disable. | P1 |
| FR-APP-13 | Original theme tokens with live preview, import/export as documented JSON, duplicate/reset and separate editor/grid/sidebar fonts, line height and density; enforce accessible contrast and reduced-motion settings. Git theme installation is deferred. | P1 |
| FR-APP-14 | Native menus, file-open associations for supported local database/SQL/workspace files, reveal-in-file-manager actions and platform-appropriate keyboard shortcuts; reject unexpected file content without executing it. | P1 |

## 2. Non-functional requirements

Targets are p95 on the reference hardware unless stated. Each has a benchmark in the perf suite ([13-performance.md](13-performance.md)).

### 2.1 Performance (`NFR-PERF`)

| ID | Target |
| --- | --- |
| NFR-PERF-01 | Cold start to interactive window ≤ 800 ms; warm start ≤ 400 ms. |
| NFR-PERF-02 | Idle RSS (app + webview processes), one connection, one tab ≤ 200 MB. |
| NFR-PERF-03 | First rows painted ≤ 50 ms after the first result batch reaches the core. |
| NFR-PERF-04 | Grid scrolling at display refresh rate with 1 M rows × 20 columns loaded; no frame > 32 ms. |
| NFR-PERF-05 | Core decode throughput ≥ 1 M cells/s per core for primitive types (PostgreSQL binary format). |
| NFR-PERF-06 | Result memory overhead ≤ 1.5× the Arrow columnar size; spill to disk beyond a configurable budget (default 512 MB per app). |
| NFR-PERF-07 | Editor keystroke-to-paint ≤ 16 ms on a 5 000-line file; autocomplete popup ≤ 50 ms. |
| NFR-PERF-08 | Schema tree for 50 000 objects: first level ≤ 300 ms, search ≤ 50 ms after cache warm. |
| NFR-PERF-09 | Export streams at ≥ 80 % of the database's own send rate (CSV). |
| NFR-PERF-10 | Main/UI thread never blocked > 50 ms by app work (long-task monitor in dev builds). |

### 2.2 Reliability (`NFR-REL`)

| ID | Target |
| --- | --- |
| NFR-REL-01 | No user data loss: unsaved editor text persisted within 1 s of change; pending change sets survive app crash. |
| NFR-REL-02 | A driver panic or failure affects only its session; the app stays up. |
| NFR-REL-03 | Network loss detected within keepalive interval (default 30 s); idle sessions auto-reconnect; in-flight statements never auto-retried unless read-only and idempotent. |
| NFR-REL-04 | Every long operation is cancellable and has a timeout. |
| NFR-REL-05 | Crash-free session rate ≥ 99.8 % (opt-in reporting). |

### 2.3 Security (`NFR-SEC`)

| ID | Target |
| --- | --- |
| NFR-SEC-01 | Secrets never written to disk in plaintext, never logged, never sent to the webview except when the user explicitly reveals one. |
| NFR-SEC-02 | The webview loads only bundled assets; strict CSP; no remote scripts. |
| NFR-SEC-03 | All TLS verified by default; insecure options require explicit per-connection opt-in and display a warning badge. |
| NFR-SEC-04 | SSH host keys verified against `known_hosts`; first-use prompt with fingerprint. |
| NFR-SEC-05 | Release artifacts code-signed, notarized (macOS), update payloads signature-verified. |
| NFR-SEC-06 | Dependencies scanned for vulnerabilities and licence compatibility on every PR. |
| NFR-SEC-07 | No telemetry by default. Crash reports opt-in and scrubbed of query text and values. |

### 2.4 Usability and accessibility (`NFR-UX`)

| ID | Target |
| --- | --- |
| NFR-UX-01 | WCAG 2.2 AA contrast for all text and focus indicators in both themes. |
| NFR-UX-02 | Full keyboard operability; visible focus; screen-reader labels; grid exposes active cell to assistive tech. |
| NFR-UX-03 | Respect OS settings: reduced motion, increased contrast, font scaling. |
| NFR-UX-04 | Every destructive action is undoable or confirmed with a clear description of impact. |

### 2.5 Portability and size (`NFR-PORT`)

| ID | Target |
| --- | --- |
| NFR-PORT-01 | macOS 12+ (arm64, x86_64), Windows 10 1809+/11 (x64, arm64), Linux x64/arm64 with WebKitGTK 4.1. |
| NFR-PORT-02 | Installer ≤ 40 MB; no bundled proprietary client libraries. |
| NFR-PORT-03 | Core crates compile and pass tests on all three OSes from day one, even before UI ships there. |

### 2.6 Maintainability (`NFR-MAINT`)

| ID | Target |
| --- | --- |
| NFR-MAINT-01 | Adding a new SQL driver requires implementing the driver traits and passing the conformance suite; no UI changes for baseline features. |
| NFR-MAINT-02 | Typed IPC: TypeScript bindings generated from Rust; no hand-written duplicates. |
| NFR-MAINT-03 | Core logic test coverage ≥ 80 % lines for `openrows-core`, `openrows-sql`, drivers' value codecs. |
