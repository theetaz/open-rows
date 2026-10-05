# 13 — Performance

Performance is a product requirement, not a polish task. Every target in [02-requirements.md](02-requirements.md#21-performance-nfr-perf) has an automated benchmark; regressions fail CI.

## 1. Reference hardware

| Name | Machine | Role |
| --- | --- | --- |
| **mac-low** | MacBook Air M1, 8 GB RAM, macOS 14 | Primary target — if it is fast here, it is fast everywhere on Mac |
| **mac-intel** | 2019 Intel i5 MacBook Pro, 8 GB | Intel floor |
| **win-mid** | 4-core x64 laptop, 8 GB, Windows 11 | Windows target (pre-1.0) |
| **linux-mid** | Same class, Ubuntu LTS, WebKitGTK 4.1 | Linux target (pre-1.0) |
| **ci-linux** | GitHub-hosted runner | Regression detection (relative, not absolute) |

Database servers for benchmarks run locally in containers (to remove network variance) plus one "remote" profile with 30 ms injected latency (`tc netem` / toxiproxy).

## 2. Performance budgets

### 2.1 Startup

Cold start budget on mac-low (target ≤ 800 ms):

| Phase | Starts at | Budget |
| --- | --- | --- |
| Process start + dynamic linking | 0 ms | 80 ms |
| Tauri init + window create (hidden) | 80 ms | 100 ms |
| WebContent process spawn + load HTML | 180 ms | 150 ms |
| Parse + execute initial JS bundle | 330 ms | 200 ms |
| First render (shell, sidebar), window shown | 530 ms | 120 ms |
| Restore tab descriptors (data loads lazily after) | 650 ms | 130 ms |
| **Interactive** | **≤ 780 ms** | |

Techniques:

- Window shown only after first paint is ready (no white flash); background colour matches theme.
- JS bundle split: shell + sidebar + last-used tab type in the initial chunk (target ≤ 350 KB gzip); editor, grid, ER diagram, API client, settings loaded on demand and preloaded during idle.
- Core initialisation is lazy: drivers register cheaply; no connection opens until needed; local store opened with WAL and a single fast query for startup state.
- Restored tabs render their descriptors immediately; data loads after first paint.
- No synchronous keychain access on startup.

### 2.2 Query to first paint

| Phase | Budget (local DB) |
| --- | --- |
| UI → core command dispatch | ≤ 2 ms |
| Statement prep + safety classification | ≤ 3 ms |
| Database time | — (not ours) |
| First batch decode (≤ 200 rows) | ≤ 5 ms |
| Event to UI + window request + Arrow encode | ≤ 5 ms |
| Worker decode + canvas paint | ≤ 10 ms |
| **Total app overhead** | **≤ 25 ms** |

### 2.3 Memory

| Component | Budget |
| --- | --- |
| Rust core baseline | ≤ 40 MB |
| Webview (WebContent + GPU + networking processes on macOS) | ≤ 140 MB at idle with one tab |
| Per open (unmounted) tab | ≤ 200 KB in webview |
| Result store | Configurable global budget, default 512 MB, spill beyond |
| Per session (driver buffers) | ≤ 4 MB typical |

## 3. Techniques by layer

### 3.1 Drivers and decoding

- Decode directly into Arrow builders; no per-cell heap allocation for primitives; strings appended into contiguous buffers.
- PostgreSQL binary format for known types (ints, floats, bool, timestamps, uuid, numeric, bytea, arrays); text format only for unknown/extension types.
- Pre-size builders from the previous batch's sizes; reuse buffers across batches where the API allows.
- Batch size adaptive: start small for fast first paint (≤ 200 rows), grow to ~64 k cells / 4 MB.
- ClickHouse and DuckDB return Arrow natively — pass through without re-encoding.

### 3.2 Core

- Zero-copy slicing for windows; Arrow IPC writer with no compression for IPC (LZ4 for spill files).
- Rayon for sort/filter/export encoding with cooperative cancellation.
- `mimalloc` as global allocator (benchmarked against system allocator in M0).
- Avoid `Arc<Mutex<…>>` hot paths: actors + message passing; `parking_lot` where locks are needed.
- `tracing` spans compiled to near-zero cost when disabled; no string formatting on hot paths unless the level is enabled.

### 3.3 IPC

- Binary responses for data; JSON only for small control messages.
- Event coalescing (progress ≤ 10 Hz).
- Tile prefetch in scroll direction; LRU cache; request deduplication.

### 3.4 UI

- Canvas grid; DOM only for chrome and overlays.
- Tab virtualisation (unmount inactive tabs).
- Workers for decoding and formatting.
- React Compiler; components subscribe to narrow Zustand selectors.
- No CSS layout animations; `content-visibility: auto` for long static panels.
- Fonts preloaded; avoid layout shift.

### 3.5 Schema and autocomplete

- Schema snapshots persisted → instant tree on reconnect.
- Bulk `search_index` names in one query; fuzzy search with a pre-built trigram/character-bag index in a worker.

## 4. Benchmark suite

| Benchmark | Tool | Metric | Gate |
| --- | --- | --- | --- |
| `decode_pg_*` (int, text, mixed, wide) | `criterion` | cells/s | −5 % vs main fails |
| `result_window_encode` | `criterion` | µs per 256×20 tile | ≤ 1 ms |
| `sql_split_*` (10 KB, 1 MB scripts) | `criterion` | MB/s | −10 % fails |
| `startup_cold` | custom harness launching the app, measuring `first-paint` mark via IPC | ms | ≤ budget on mac-low (nightly), relative on CI |
| `grid_scroll_1m` | Playwright + Performance API on synthetic source | long frames, avg frame time | 0 frames > 32 ms |
| `query_1m_rows_pg` | integration harness against container | time to first paint, total time, peak RSS | within budgets |
| `editor_type_5k_lines` | Playwright | input latency p95 | ≤ 16 ms |
| `idle_memory` | harness reads RSS of all app processes after 60 s | MB | ≤ 200 MB |
| `export_csv_10m` | integration | MB/s vs `psql \copy` baseline | ≥ 80 % |

Results are stored per commit (JSON in a `perf-results` branch or artefact store) and charted; PRs get a comment with deltas.

## 5. Profiling playbook

| Symptom | Tool |
| --- | --- |
| Rust CPU hot spots | `cargo flamegraph` / `samply` (macOS-friendly, opens in Firefox Profiler) |
| Async stalls | `tokio-console` (dev builds with `tokio_unstable`) |
| Allocation churn | `dhat-rs` in benchmarks, Instruments Allocations on macOS |
| Webview JS/paint | Safari Web Inspector (WKWebView, enable via dev build `devtools` feature); Chrome DevTools on Windows (WebView2) |
| Long tasks | Dev-build long-task observer logs any main-thread task > 50 ms with a stack |
| Memory in webview | Safari Timelines → JavaScript Allocations; heap snapshots |
| Startup | `startup trace` mode: timestamps from process start through first paint written to log |

## 6. Performance anti-patterns (review checklist)

- Sending rows as JSON over IPC.
- Holding result rows in React state or Zustand.
- Calling `measureText` per cell per frame.
- Running introspection on a user's query session.
- `KEYS *`, `SELECT COUNT(*)` on large tables without user intent (use estimates: `pg_class.reltuples`, `information_schema.TABLES.TABLE_ROWS`, flagged as estimates).
- Synchronous file or keychain I/O on the main thread.
- Unbounded channels or buffers anywhere in the data path.
