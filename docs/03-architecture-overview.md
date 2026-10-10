# 03 — Architecture Overview

## 1. Architectural drivers

The shape of the system follows from a handful of requirements:

| Driver | Consequence |
| --- | --- |
| Native-class speed with a web-tech UI | All heavy work in Rust; UI is a thin viewport that pulls only what it displays. |
| Many database paradigms (SQL, document, KV, wide-column, search) | One driver contract with capability flags; a shared value model; paradigm-specific views built on shared primitives. |
| Results of unbounded size | Results stored columnar (Arrow) in the core, spilled to disk past a budget; UI fetches windows. |
| Robustness | Actor-per-session isolation, panic containment, cancellation everywhere, optional out-of-process host for FFI drivers. |
| Cross-platform, macOS first | Tauri 2 + system webview; platform specifics isolated behind small traits. |
| Headless reuse (CLI, tests) | Core has zero dependency on Tauri; the app is just one front-end. |

## 2. System context

```mermaid
flowchart LR
    user(["User"])
    subgraph machine["User's machine"]
        app["OpenRows<br/>desktop app"]
        cli["openrows CLI<br/>(headless runner)"]
        keychain[("OS keychain")]
        files[("Project folder<br/>(git repo)")]
        localdata[("App data dir<br/>history, cache, settings")]
        tools["User-installed tools<br/>pg_dump, mysqldump,<br/>Oracle Instant Client"]
    end
    dbs[("Databases<br/>PG, MySQL, SQLite, MSSQL,<br/>Oracle, Mongo, Redis, ...")]
    apis["HTTP / GraphQL /<br/>WebSocket / gRPC APIs"]
    bastion["SSH bastion hosts"]
    updates["Release server<br/>(GitHub Releases)"]

    user --> app
    user --> cli
    app --> keychain
    app --> files
    app --> localdata
    app --> tools
    cli --> files
    app -->|TLS| dbs
    app -->|SSH tunnel| bastion --> dbs
    app --> apis
    cli --> apis
    app -->|signed update check| updates
```

Nothing in this diagram is a server we operate, except the static release feed.

## 3. Containers

```mermaid
flowchart TB
    subgraph tauri["Desktop app process (Rust)"]
        direction TB
        shell["App shell<br/>openrows-app (Tauri 2)<br/>windows, menus, updater, IPC router"]
        subgraph core["Core engine (no Tauri dependency)"]
            sessions["Session manager<br/>openrows-core"]
            results["Result store<br/>openrows-results (Arrow + spill)"]
            sql["SQL toolkit<br/>openrows-sql"]
            schema["Schema cache<br/>openrows-schema"]
            http["API engine<br/>openrows-http"]
            script["Script sandbox<br/>openrows-script (QuickJS)"]
            net["Network layer<br/>openrows-net (SSH, TLS, proxy)"]
            store["Local store<br/>openrows-store (SQLite)"]
            secrets["Secrets<br/>openrows-secrets (keychain)"]
            ai["AI + MCP<br/>openrows-ai, openrows-mcp"]
            drivers["Drivers<br/>openrows-driver-*"]
        end
        shell --> sessions
        shell --> results
        shell --> http
        sessions --> drivers
        sessions --> sql
        sessions --> schema
        drivers --> net
        http --> net
        http --> script
        sessions --> results
        schema --> store
        sessions --> secrets
        shell --> ai
        ai --> schema
        ai --> sessions
        ai --> net
    end

    subgraph web["Webview process (WKWebView / WebView2 / WebKitGTK)"]
        ui["React UI<br/>shell, sidebar, tabs"]
        grid["Canvas data grid"]
        editor["CodeMirror 6 editor"]
        workers["Web Workers<br/>Arrow decode, formatting"]
    end

    subgraph host["Optional driver host process (Rust)"]
        ffi["FFI drivers<br/>Oracle (ODPI-C), DuckDB,<br/>ADBC drivers"]
    end

    ui <-->|"commands (JSON)"| shell
    grid <-->|"Arrow IPC (binary)"| shell
    ui <-->|"events / channels"| shell
    sessions <-->|"local socket, framed msgs"| ffi
```

### 3.1 Responsibilities

| Container | Owns | Does not own |
| --- | --- | --- |
| **App shell** (`openrows-app`) | Tauri setup, window lifecycle, native menus, IPC command registration, capability permissions, updater, mapping IPC to core APIs | Any database or HTTP logic |
| **Core engine** | Connections, sessions, query execution, result storage, schema caching, HTTP engine, scripting, secrets, persistence | UI concerns, rendering, Tauri types |
| **Webview UI** | Presentation, interaction, layout, ephemeral view state | Long-lived data, credentials, protocol logic |
| **Driver host** (optional) | Running C/C++ driver libraries out of process so a segfault cannot take down the app | Anything else |

## 4. Process and thread model

```mermaid
flowchart LR
    subgraph main["App process"]
        mt["Main thread<br/>(OS event loop, Tauri, menus)"]
        subgraph rt["tokio multi-thread runtime"]
            ipc["IPC handlers"]
            actors["Session actors<br/>(1 task per session)"]
            netio["Network I/O<br/>(SSH, TLS, HTTP)"]
        end
        blocking["Dedicated OS threads<br/>for blocking drivers<br/>(SQLite, DuckDB)"]
        cpu["Rayon pool<br/>(sort, filter, export encode)"]
    end
    subgraph wv["Webview processes"]
        uimain["UI main thread"]
        ww["Web Workers"]
    end
    mt --> ipc
    ipc --> actors
    actors --> netio
    actors --> blocking
    actors --> cpu
    uimain <--> ipc
    uimain <--> ww
```

Rules:

1. **The OS main thread never waits on I/O.** Tauri command handlers are `async` and hop onto the tokio runtime.
2. **One actor per session.** A session (a live database connection) is owned by a single tokio task that processes commands from an `mpsc` queue. This serialises access to the connection without locks and makes state transitions explicit. See [04-core-engine.md](04-core-engine.md#3-session-actors).
3. **Blocking drivers get their own thread**, not the shared `spawn_blocking` pool, so a long SQLite query cannot starve unrelated work and so thread-affine libraries keep their thread.
4. **CPU-heavy work** (client-side sort of 5 M rows, Parquet encoding) runs on a bounded Rayon pool, never on the tokio workers.
5. **The UI main thread only renders and handles input.** Arrow decoding, JSON pretty-printing of large values, and diff computation go to Web Workers.

## 5. Crate and package map

```mermaid
flowchart BT
    types["openrows-types<br/>value model, ids, errors"]
    driverapi["openrows-driver-api<br/>traits, capabilities"]
    net["openrows-net"]
    secrets["openrows-secrets"]
    sqlk["openrows-sql"]
    resultsk["openrows-results"]
    storek["openrows-store"]
    schemak["openrows-schema"]
    corek["openrows-core"]
    httpk["openrows-http"]
    scriptk["openrows-script"]
    pg["openrows-driver-postgres"]
    my["openrows-driver-mysql"]
    lite["openrows-driver-sqlite"]
    more["openrows-driver-* ..."]
    app["openrows-app (Tauri)"]
    cli["openrows-cli"]
    hostk["openrows-driver-host"]

    driverapi --> types
    resultsk --> types
    sqlk --> types
    net --> types
    pg --> driverapi
    pg --> net
    my --> driverapi
    my --> net
    lite --> driverapi
    more --> driverapi
    schemak --> driverapi
    schemak --> storek
    corek --> driverapi
    corek --> resultsk
    corek --> sqlk
    corek --> schemak
    corek --> secrets
    httpk --> net
    httpk --> scriptk
    app --> corek
    app --> httpk
    cli --> corek
    cli --> httpk
    hostk --> driverapi
```

Dependency rules (enforced in CI with `cargo-deny` bans and a custom check):

- `openrows-types` depends on nothing internal.
- Drivers depend only on `openrows-driver-api`, `openrows-types`, `openrows-net`. Drivers never depend on each other or on `openrows-core`.
- Nothing in `crates/` depends on Tauri. Only `apps/desktop/src-tauri` does.
- Each driver is behind a Cargo feature so slim builds (e.g. the CLI) can exclude drivers.

The front-end packages mirror this: `@openrows/ipc` (generated bindings) ← `@openrows/grid`, `@openrows/editor`, `@openrows/ui` ← `apps/desktop/src`.

## 6. Key flows

### 6.1 Open a table

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant UI as React UI
    participant SH as App shell (IPC)
    participant SM as Session actor
    participant DR as PostgreSQL driver
    participant RS as Result store
    participant DB as PostgreSQL

    U->>UI: double-click table "orders"
    UI->>SH: table.open(conn, "public.orders", page=1)
    SH->>SM: Execute(SELECT ... LIMIT 1000)
    SM->>DR: execute(stmt, sink)
    DR->>DB: Bind/Execute (binary format)
    DB-->>DR: RowDescription + DataRows
    DR-->>RS: RecordBatch #1 (≤ 64k cells)
    RS-->>SH: event result.batch(queryId, rowCount, schema)
    SH-->>UI: channel message
    UI->>SH: result.window(queryId, rows 0..60, cols 0..12)
    SH->>RS: slice + encode Arrow IPC
    RS-->>UI: ArrayBuffer (Arrow IPC)
    UI->>UI: worker decode, paint canvas
    DR-->>RS: remaining batches, CommandComplete
    RS-->>UI: result.complete(totalRows, elapsed)
```

### 6.2 Commit an edit

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant UI as React UI
    participant CS as Change set (core)
    participant SM as Session actor
    participant DB as Database

    U->>UI: edit cells, add a row, delete a row
    UI->>CS: stage(edits)  (persisted to local store)
    U->>UI: Commit (Cmd+S)
    UI->>CS: preview()
    CS-->>UI: generated SQL + affected row estimate
    U->>UI: confirm (prod connections always ask)
    UI->>SM: apply(changeSetId)
    SM->>DB: BEGIN
    loop each statement
        SM->>DB: UPDATE/INSERT/DELETE ... WHERE pk = $1 [AND original values]
        DB-->>SM: rows affected
    end
    alt all rows affected as expected
        SM->>DB: COMMIT
        SM-->>UI: success, refreshed rows
    else conflict or error
        SM->>DB: ROLLBACK
        SM-->>UI: error with failing statement, change set kept
    end
```

## 7. Technology choices

| Concern | Choice | Why | Alternatives considered |
| --- | --- | --- | --- |
| App shell | Tauri 2 | Small binaries, system webview, Rust back end, mature plugin set, multi-window | Electron (memory/size), native per OS (3× UI cost), Flutter (weaker text/editor ecosystem) |
| Async runtime | tokio | De-facto standard; all chosen drivers support it | async-std (declining) |
| Result format | Apache Arrow (`arrow-rs`, `apache-arrow` JS) | Columnar, zero-copy slicing, typed, cross-language, Parquet export for free | JSON (slow, large), custom binary (more code to own) |
| SQL parsing | `sqlparser-rs` + fallback tokenizer | Multi-dialect, Apache-2.0, widely used | Tree-sitter grammars (good for highlighting, weaker AST), hand-written |
| Editor | CodeMirror 6 | Lighter than Monaco, modular, excellent large-doc performance, mobile-friendly IME | Monaco (heavy, VS Code-centric) |
| Data grid | Canvas renderer (see ADR-0004) | DOM grids cannot sustain 60 fps at 1 M rows with rich cells | DOM virtualisation libraries |
| UI framework | React 19 + TypeScript + Vite | Ecosystem, hiring, tooling; compiler-based memoisation | Svelte/Solid (smaller ecosystem for complex widgets) |
| Styling | CSS variables design tokens + CSS modules; Radix primitives for accessible behaviour | Theming, no runtime CSS-in-JS cost | Tailwind (fine, but tokens + modules keep markup cleaner for dense UI) |
| State | Zustand (UI state), TanStack Query (IPC-backed data) | Small, explicit, good DevTools | Redux (boilerplate) |
| IPC bindings | `tauri-specta` → TypeScript | One source of truth for types | Hand-written types |
| Local store | SQLite via `rusqlite` (WAL, FTS5) | Embedded, robust, searchable history | sled/redb (no SQL/FTS) |
| SSH | `russh` | Pure Rust, async, agent + jump support | libssh2 bindings (C, blocking) |
| TLS | `rustls` + `rustls-platform-verifier` | Memory-safe, uses OS trust store | OpenSSL (packaging pain on macOS/Windows) |
| Secrets | `keyring` crate (+ `security-framework` for macOS extras) | Cross-platform keychain access | Own encrypted vault (key management burden) |
| HTTP engine | `hyper` 1.x with custom connector, `reqwest` for simple paths | Need per-phase timings, HTTP/2, proxy, client certs | curl bindings (C dependency) |
| gRPC | `tonic` + `prost-reflect` | Dynamic messages from reflection or `.proto` | grpcurl sidecar |
| Scripting | QuickJS via `rquickjs` | Small, embeddable, sandboxable with memory/time limits | Boa (slower), V8 (huge), Deno core (heavy) |
| Logging | `tracing` + rolling file appender | Structured, spans across async | log/env_logger |

Each non-obvious choice has an ADR in [adr/](adr/README.md).

## 8. Cross-cutting concerns map

| Concern | Document |
| --- | --- |
| Security & secrets | [12-security.md](12-security.md) |
| Performance budgets | [13-performance.md](13-performance.md) |
| Errors, reconnection, logs | [14-reliability-and-observability.md](14-reliability-and-observability.md) |
| Persistence & migrations | [15-storage-and-persistence.md](15-storage-and-persistence.md) |
| Platform specifics | [16-platform-and-distribution.md](16-platform-and-distribution.md) |

## 9. Implemented (M2-01c): installed host boundary

The desktop manages a lazy `openrows_host_client::DriverHostRuntime` alongside
the existing sessions. It resolves the unsuffixed helper beside `current_exe`,
uses private 0700 rendezvous directories under the app cache's `hosts/`, and
shares 256 MiB per-host / 512 MiB aggregate admission and profile/driver crash
circuits. No helper starts during app setup. The desktop now depends on the
Tauri-free client/protocol, with no native hosted engine dependency.

Tauri stages/bundles a real production helper for each target. The closed
`HostDriverId` is the only argument; the diagnostic Probe has no database
capabilities. SQLite remains a test feature. M2-04 adds DuckDB's ID, host-only
registry/contract and parent metadata adapter; there is no hosted driver in the
picker yet. Installed/signing evidence and platform gaps are in [ADR-0010](adr/0010-driver-host-process.md).
