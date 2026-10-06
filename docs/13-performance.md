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

Total idle physical footprint across the app and all its webview processes must be ≤ 200 MB (median of sequential launches)
(NFR-PERF-02), in addition to the component targets below. Physical footprint
(the Activity Monitor metric) avoids summed RSS double-counting shared system
and WebKit pages.

| Component | Budget |
| --- | --- |
| Rust core baseline | ≤ 40 MB |
| Webview (WebContent + GPU + networking processes on macOS) | ≤ 140 MB physical footprint at idle with one tab |
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
- Tab virtualisation (unmount inactive tabs; retain descriptors, SQL drafts and view state).
- The current mock DOM grid mounts viewport rows plus three rows of overscan on each edge,
  observes viewport resizing and scrolls keyboard selection into the mounted range.
- Persistent panels/toolbars use tone tokens over native macOS vibrancy, without CSS
  backdrop filters or compositing hints. Blur is confined to mounted palettes, sheets
  and rail tooltips; a stylesheet allowlist test in `just check` guards this boundary.
- Closed split panels unmount after the 360 ms exit (immediately under reduced motion);
  palette exit retains DOM for at most 160 ms. Gallery unmounts when closed; the workspace
  keeps its controller state while its content DOM is unmounted during gallery visits.
- Non-initial gallery, palettes, review, SQL views and inspector modes use dynamic imports.
  Palette and sheet primitives have separate `@openrows/ui/palette` and `@openrows/ui/sheet`
  entry points so the shell barrel cannot pull their code/styles into the initial graph.
  `just bench-ui` sums all manifest entry/static-import chunks (deduplicated), excluding
  dynamic chunks, and enforces the 350,000-byte gzip limit. `just check` runs this
  same gate. `packages/ui/scripts/bundle-size.ts` also checks emitted chunk-module
  metadata to reject `apache-arrow` in that static graph; configure package names
  with the comma-separated `BUNDLE_DENY_MODULES` environment variable (default:
  `apache-arrow`). Vite only emits evidence; all assertions live in the script.
  The default wide-window Assistant remains visible and loads on that first render.
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
| `result_window_encode` | `criterion`, `just bench-result-encode` | µs per tile; local M4: 256×20 11.49, 1024×20 17.32, 256×100 46.27 | 256×20 mean upper 95% confidence bound ≤ 1 ms; M4 pass; revisit on mac-low |
| `ipc_tile_round_trip` (M0 S1) | `just bench-ipc` after `just build-app`; 500 unique random tiles, warm worker, full client/worker message path | M4 runs 1/2 (p50/p95/max ms): invoke 0/1/6 both; decode 0/1/4 and 0/1/3; total 0/1.00/6 both; run 2 total mean 0.524 ms | total p95 ≤ 3 ms: M4 PASS both runs; 1 ms page clock, isolation false; revisit on mac-low |
| `sql_split_*` (10 KB, 1 MB scripts) | `criterion` | MB/s | −10 % fails |
| `startup_cold` | `just bench-startup [runs=10]` after `just build-app`; sequential release processes, shared monotonic phase timestamps, spawn → window shown | per-phase min / p50 / p95 / max (ms) | warm runs 2..N p50 ≤ 400 ms; all-runs p95 ≤ 800 ms (NFR-PERF-01); macOS GUI only, not Linux CI |
| `grid_scroll_1m` | Playwright + Performance API on synthetic source | long frames, avg frame time | 0 frames > 32 ms |
| `query_1m_rows_pg` | integration harness against container | time to first paint, total time, peak RSS | within budgets |
| `editor_type_5k_lines` | Playwright | input latency p95 | ≤ 16 ms |
| `idle_memory` | `just bench-memory [runs=3]` after `just build-app`; first-paint receipt + 5 s settle, `ps` RSS for app + descendants + responsible WebKit XPC services; required `footprint -f bytes` | per-run totals, per-process medians and informational RSS in decimal MB | median total physical footprint ≤ 200 MB; median WebContent + GPU + Networking ≤ 140 MB (NFR-PERF-02); macOS GUI only, not Linux CI |
| `export_csv_10m` | integration | MB/s vs `psql \copy` baseline | ≥ 80 % |

The startup, memory and IPC harnesses use Python 3's standard library and build nothing.
They write timestamped JSON to the gitignored `perf-results/startup-*.json` and
`perf-results/memory-*.json` (IPC: `perf-results/ipc-*.json`), including samples, methods, machine metadata and
budget verdicts. A missed budget or invalid measurement returns a nonzero status.
`just bench-test` tests statistics, budgets, report validation, process attribution
and cleanup without opening the application; it is part of `just check`.

The first sample is labelled **first launch**, not OS-cold. Run immediately after
`just build-app` for a first launch of that build. True OS-cold measurement requires
a reboot, then running the already-built executable before another OpenRows launch.
There is no cache purge or privileged command. Runs 2..N are warm. p50 is the median;
p95 uses nearest rank (with ten runs it is the maximum). These are local maintainer
gates; CI integration awaits a macOS GUI runner.

When `OPENROWS_STARTUP_REPORT` is set, Rust records absolute `CLOCK_MONOTONIC`
nanoseconds for main entry, builder start, main webview creation, setup completion,
page-load completion, first-paint receipt and return from `window.show()`. Normal
launches do not install benchmark hooks, read the clock, create threads or write
reports. The main webview is created before Tauri's application setup callback.
The phase table shows intervals ending at each milestone in actual timestamp order;
JSON also includes cumulative elapsed times. A late page-load callback does not
extend the window-show total. Callbacks can overlap; these intervals
are milestones, not exclusive CPU costs. Window-show return is a paint-ready proxy,
not a compositor timestamp.

Memory runs launch sequentially, completing guarded process cleanup before the next launch.
The default is three; a positive explicit count supports diagnostic single runs and even counts.
Each run reports app and webview footprint totals; JSON retains process samples and startup
reports. Both budget gates take the median of complete per-run sums, never the sum of
per-process medians. Process medians group duplicate names within a run and count an absent
process name as zero in other complete runs. Any invalid footprint fails the whole report,
even if other runs would give a passing median. RSS remains informational.

M0-T6 / issue #17 real-Mac footprint and visual parity are pending maintainer measurement;
source and synthetic tests do not certify either physical-footprint budget.

Memory sampling uses an event-driven report wait and a timed Python wait until
five seconds after first paint. It counts WebContent, GPU and Networking even when
launchd is their parent, using macOS's exported libSystem responsibility query.
When spawned by the harness, the app can inherit responsibility from the launching
terminal or GUI host. WebKit XPC services are attributed when their responsible
PID matches the app root's responsible PID or an already selected app process,
and their `ps lstart` time is at or after the root's start time. Start times are
parsed chronologically at one-second resolution; older host-owned WebKit services
are excluded. Cleanup retains start-time/name identity guards against PID reuse.
The responsibility symbol is not a public API: if unavailable, or no owned WebContent is found,
the benchmark fails rather than certifying an incomplete memory total. Physical
footprint must be measured for every attributed process; unavailable, invalid or
permission-denied measurements fail explicitly, with no RSS fallback. JSON keeps
per-run totals; the gates identify `median_total_footprint_bytes` and
`median_webview_footprint_bytes`. RSS is
informational and sums shared mappings without deduplication; MB means 1,000,000 bytes. The current M0 workspace mock
has no live connection implementation, so its idle result is a preview baseline, not yet
the one-connection/one-tab workload in NFR-PERF-02.

Each instance receives a benchmark-only stop-file request for Tauri's normal exit.
Timeouts escalate to TERM then KILL of its isolated process group; surviving owned
XPC processes are signalled by PID with start-time/name guards against reuse.
Survivors cause a cleanup failure. Temporary reports, partial writes and stop files
live in a context-managed directory and are removed on success, errors and Ctrl-C.
Retained performance JSON is intentional. Do not force-kill the harness itself;
SIGKILL cannot execute cleanup.

### M0 S1 measurement boundary

The encoder numbers above were measured on a Mac mini M4 / 24 GB, not mac-low.
There is no main baseline for the new encoder. The Arrow transport decision is
accepted after two maintainer release WKWebView runs of 500 tiles each; see [ADR-0002](adr/0002-arrow-ipc-for-results.md#m0-s1-evidence--2026-10-06)
for the workload, command, confidence intervals, binary typing and dependency
rationale. Both runs measured total p95 1.00 ms against the 3 ms limit.
The page timer resolution was 1.0 ms, with `crossOriginIsolated: false`; ineffective
COOP/COEP configuration was removed. With this quantisation, p95 ≤ 1 ms implies
true p95 < 2 ms. Run 2 mean total was 0.524 ms across 500 quantised samples,
supporting acceptance without implying finer timer precision. Per-stage means
were not supplied for those runs. Revisit on mac-low and real driver data.

`OPENROWS_SPIKE=ipc` is an explicit release-app measurement mode.
`OPENROWS_SPIKE_REPORT` receives an atomic JSON report and the app then exits.
The ordinary startup path does not generate synthetic rows or start an Arrow
worker. IPC total time starts at the tile client request and ends when typed
columns are returned; it includes the two worker messages and cache insertion.
Invoke and decode use the same page clock as total. Decode is measured from
posting bytes to receiving typed columns and includes both transfers; the
worker's internal decode is retained as informational `workerDecodeMs`.
Library/worker startup is excluded, but all 500 samples (including the first
actual decode) are retained. Offsets are unique so the deduplicating 64-tile LRU
cannot hide a cache hit. `just bench-test` validates the IPC and encoding gates
without launching the app. If a real run fails, profile before implementing the
minimal custom columnar alternative in ADR-0002.

Before sampling, the page probes 100000 consecutive `performance.now()` reads
and records the minimum positive delta in `timer.resolutionMs`, plus the actual
`timer.crossOriginIsolated` flag. No observed tick is reported as null and fails
validation. These are observed timer properties, not a promise of precision on
every WebKit version. The stage-containment check permits invoke + decode to
exceed total by at most twice that measured resolution (plus floating-point
comparison precision); it still rejects missing, negative or nonfinite timings,
wrong workloads, partial runs and duplicate offsets. Worker time is excluded
from this sum. The total p95 gate remains exactly 3 ms with no rounding allowance.
The harness prints resolution/isolation and p50/p95/max/mean for all four timings.
JSON `stage_summary_ms` also includes each stage’s arithmetic mean of the
quantised samples; worker means remain informational.
It retains JSON even after validation, app-exit or cleanup failure, with raw
samples, diagnostic summaries and `verdict`/`error`; `spike_error` preserves the
original page failure. Diagnostic summaries on invalid runs do not certify a
passing benchmark.


#### M0 S1 macOS failure diagnostics

The frontend queries spike mode through `spike_start`; the command logs page
script entry and the spike flag, shows and focuses the native window, and only
then returns permission to load the spike and create its worker. This removes
reliance on an injected global and keeps the measured workload out of hidden
WKWebView startup. Normal mode returns false and loads no worker.

Release workers use Vite's `?worker&inline` constructor and `iife` format, with a
static decoder import bundled into the blob. No worker module or decoder fetch
uses `tauri://localhost`. CSP allows `blob:` for `worker-src` and `script-src`;
`data:` scripts and `unsafe-eval` remain disallowed. Arrow stays outside the
initial JS graph; the spike chunk remains lazy.

Startup handshake/module loading and worker initialization have 10-second
watchdogs; every full tile request (invoke plus worker decoding) has a 2-second
watchdog. Window errors, resource errors and unhandled rejections produce one
terminal error report. Progress logs native setup, page script entry, mode,
window shown, worker creation/readiness, first request/bytes/decode, every 100
samples and finishing. Diagnostic IPC is queued without waiting inside sample
measurements; diagnostic commands also have a 2-second watchdog.

Each milestone appends one line to the report path with `.progress` replacing
`.json`, and to stderr. The harness captures stderr in its temporary directory
and prints both tails (last 30 lines, at most 8192 bytes per file) on timeout,
early exit or invalid report. Reports, progress and stderr are removed with the
existing context-managed cleanup. These changes were tested without launching
the app. The subsequent maintainer M4 runs passed; mac-low validation remains
a revisit point.


## 5. Profiling playbook

| Symptom | Tool |
| --- | --- |
| Rust CPU hot spots | `cargo flamegraph` / `samply` (macOS-friendly, opens in Firefox Profiler) |
| Async stalls | `tokio-console` (dev builds with `tokio_unstable`) |
| Allocation churn | `dhat-rs` in benchmarks, Instruments Allocations on macOS |
| Webview JS/paint | Safari Web Inspector (WKWebView, enable via dev build `devtools` feature); Chrome DevTools on Windows (WebView2) |
| Long tasks | Dev-build long-task observer logs any main-thread task > 50 ms with a stack |
| Memory in webview | Safari Timelines → JavaScript Allocations; heap snapshots |
| Startup | `just bench-startup`: monotonic milestones through window shown in `perf-results/startup-*.json` |

## 6. Performance anti-patterns (review checklist)

- Sending rows as JSON over IPC.
- Holding result rows in React state or Zustand.
- Calling `measureText` per cell per frame.
- Running introspection on a user's query session.
- `KEYS *`, `SELECT COUNT(*)` on large tables without user intent (use estimates: `pg_class.reltuples`, `information_schema.TABLES.TABLE_ROWS`, flagged as estimates).
- Synchronous file or keychain I/O on the main thread.
- Unbounded channels or buffers anywhere in the data path.
