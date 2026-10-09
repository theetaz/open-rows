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
  metadata to reject heavy modules in that static graph. The required list is
  `apache-arrow`, `packages/grid/src/`, `packages/editor/src/`, `@codemirror`,
  `@lezer` and `@replit/codemirror-vim`. Add more with the comma-separated
  `BUNDLE_DENY_MODULES` environment variable. With the S5 editor, the initial JS is
  95,222 gzip bytes; CodeMirror + PostgreSQL is ≈ 123 KB gzip in lazy chunks and the Vim chunk
  ≈ 41 KB gzip. Vite only emits evidence; all assertions live in the script.
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
| `decode_pg_*` (int, text, mixed, wide, first_batch) (M0 S3) | `criterion`, `just bench-pg-decode`; synthetic PG binary `DataRow` frames, no network | cells/s per core from the mean's upper 95% bound; local M4 (M cells/s): int 330, text 62.9, mixed 139, wide 169; first_batch (20×200) 0.036 ms | each ≥ 1 M cells/s/core (NFR-PERF-05) and first batch ≤ 5 ms: M4 pass; −5 % vs main once CI compares; revisit on mac-low |
| `result_window_encode` | `criterion`, `just bench-result-encode` | µs per tile; local M4: 256×20 11.49, 1024×20 17.32, 256×100 46.27 | 256×20 mean upper 95% confidence bound ≤ 1 ms; M4 pass; revisit on mac-low |
| `ipc_tile_round_trip` (M0 S1) | `just bench-ipc` after `just build-app`; 500 unique random tiles, warm worker, full client/worker message path | M4 runs 1/2 (p50/p95/max ms): invoke 0/1/6 both; decode 0/1/4 and 0/1/3; total 0/1.00/6 both; run 2 total mean 0.524 ms | total p95 ≤ 3 ms: M4 PASS both runs; 1 ms page clock, isolation false; revisit on mac-low |
| `sql_split_*` (10 KB, 1 MB scripts) | `criterion`, `just bench-sql-split`; synthetic per-dialect scripts with comments, quoted `;`, dollar quotes, `DELIMITER` and trigger blocks | MiB/s at the time's upper 95% bound; local M4 (shared with other builds), 10 KB / 1 MB: PostgreSQL 293 / 291, MySQL 186 / 121, SQLite 264 / 152 | −10 % fails once CI compares against `main`; no absolute gate |
| `schema_*` at 50 000 objects (M1-16) | `criterion`, `just bench-schema`; synthetic objects, snapshot in a temporary local store | ms from the mean's upper 95% bound; local M4 (shared with other builds): first level (relaunched cache restores one schema's 50 000-object snapshot) 37.5; warm search exact 7.04, prefix 4.85, substring 3.88, subsequence 5.35, schema filter 0.91; index build ≈ 9 (informational) | first level ≤ 300 ms and each search ≤ 50 ms (NFR-PERF-08): M4 pass; revisit on mac-low |
| `startup_cold` | `just bench-startup [runs=10]` after `just build-app`; sequential release processes, shared monotonic phase timestamps, spawn → window shown | per-phase min / p50 / p95 / max (ms) | warm runs 2..N p50 ≤ 400 ms; all-runs p95 ≤ 800 ms (NFR-PERF-01); macOS GUI only, not Linux CI |
| `grid_scroll_1m` (M0 S2) | `just bench-grid [repeats=3]` after `just build-app`; own Canvas 2D in release WKWebView, S1 tile/worker path; four scenarios per fresh launch/repeat | All repeats: p50/p95/p99/max, >20/>32 counts, frames >32 with tracked work ≥16 ms, refresh, paint, tile/cache/blank counters, raw attribution | Best repeat per scenario: 0 frames >32 ms; every repeat: 0 frames >32 ms with tracked work ≥16 ms. S2 met on supplied M4/60 Hz evidence; [accepted ADR-0004](adr/0004-canvas-data-grid.md) |
| `query_1m_rows_pg` | integration harness against container | time to first paint, total time, peak RSS | within budgets |
| `editor_type_5k_lines` (M0 S5) | `just bench-editor` after `just build-app`; CodeMirror 6 in release WKWebView, 5 000-line PostgreSQL document, 240 scripted keystrokes per configuration (`sql`: highlighting + completion; `vim`: plus Vim mode) at top/middle/end | keystroke-to-paint p50/p95/max, >16 ms count, per-phase and per-site p95, wall latency incl. vsync wait; M4 implementer runs 1–4 p95: sql 11/9/10/9 ms, vim 9/9/9/10 ms | p95 ≤ 16 ms per configuration (NFR-PERF-07): M4 PASS on all four valid runs; maintainer confirmation pending; 1 ms page clock; [ADR-0003](adr/0003-codemirror-6-editor.md) |
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


### M0 S2 grid measurement and robust gate

[ADR-0004](adr/0004-canvas-data-grid.md) accepts our own Canvas 2D painter based
on three maintainer release runs on a Mac mini M4 at 60 Hz: mean complete paint
0.5–1.9 ms/frame, zero per-frame painter allocations and zero blank-placeholder
frames. Each scenario has a supplied run with zero intervals >32 ms. Large random
stalls in both comparison renderers had zero tracked work under unrelated machine
load (load average 6–8), supporting the maintainer's environmental classification.
This does not identify a particular scheduling/compositor/GC mechanism.

`just bench-grid [repeats=3]` runs the four scenarios N times. Each repeat launches
a fresh release app; every scenario remounts at scroll position zero with a new
64-tile LRU and three unsampled mount frames. Gate **each scenario's best repeat**
(fewest >32 ms frames; lowest max interval breaks ties) on **zero frames strictly
>32 ms**. A second gate requires **zero frames >32 ms with tracked work ≥16 ms
in every repeat**, including repeats not selected as best. This prevents repeated
renderer work from being hidden by a quiet run, while allowing unrelated system
load to affect some repeats. If all repeats of a scenario stall, it still fails.

Tracked work is the inclusive sum of scroll handler, complete synchronous React
commit, canvas resize, prepare, complete paint, tile apply, main-thread conversion
and scripted driver timings. Worker conversion CPU runs off-thread and is excluded;
allocation counts are diagnostics, not milliseconds. Timings may overlap, so the
sum is deliberately conservative rather than exact CPU accounting. A zero-work
stall is retained as unexplained/environmental noise; it is never discarded or
labelled GC without a profiler trace. Exactly 32 ms does not fail; exactly 16 ms
of tracked work on a >32 ms interval does fail. Thresholds have no timer rounding
allowance. All repeats print p50/p95/p99/max, >20/>32 counts, own-cost violations,
mean paint, refresh and tile/cache/blank counters, plus their top ten attributed
intervals. JSON keeps every raw sample, report, selected-repeat gate and verdict.
Missing repeats/scenarios, short or over-budget durations, wrong observed endpoints,
nonfinite values, missing timer/counters/breakdown or ownership/cleanup failures
fail closed. Synthetic tests cover the policy without launching the app.

The fixed host is 1040×600 CSS pixels, with 26 px rows, 120 px columns and two
pinned columns. Every scenario includes a sampled two-second token/style preview:

- Ten seconds of smooth vertical scrolling at 6000 px/s, split equally among
  top, middle and near-bottom starts.
- Sixty large jumps permuted across rows 0–999,000, then sixty 574 px page-down
  steps from the middle, one command every 100 ms.
- Five seconds of horizontal scrolling at 3000 px/s across 100 virtual columns
  (twenty S1 mixed columns repeated), at row 500,000; hold at the right edge.
- Jump to row 999,000 and stop at the first observed placeholder-free frame;
  fail if it does not settle within two seconds after preview.

Non-jump scenarios hold endpoints for 500 ms. All windows traverse binary IPC and
Arrow worker decoding; the Rust fixture stays 1 M×20. Observed offsets validate
endpoints; continuous sweeps require ≤1000 px deviation from commanded offsets.
Bounded 100 ms scripted destination lookahead warms known teleport destinations
without awaiting IPC in the driver; it is benchmark input, not prediction of
arbitrary user jumps. Arrivals remain sampled. Worker-ready display cells remove
main-thread Arrow formatting; backing stores change only on observed resize/DPR.

Maximum sampling is 38.5 seconds per repeat (115.5 seconds for default N=3).
Each repeat has a 60-second scenario-sequence watchdog and a 130-second report
deadline; the total command scales with N and startup/cleanup, with no fixed
three-minute promise. Progress streams immediately; keep the window visible.
`perf-results/grid-*.json` records partial evidence even on failure. The harness
uses isolated-process ownership and guarded cleanup. Paint measures the complete
layered painter with the page clock (resolution reported, typically 1 ms), not
GPU completion. Preallocated counters become JSON arrays after sampling. React's
pinned production commit hook excludes delayed passive effects and reads no clock
in normal mode. No new native run or main baseline is claimed by this change.

### M0 S5 editor keystroke measurement

`OPENROWS_SPIKE=editor` uses the S1 handshake, watchdogs, progress file and atomic
report. The Rust shell skips the synthetic 1 M-row store in this mode. The page
lazily loads `spike/editor.ts`, `@openrows/editor` and the `@openrows/editor/vim`
chunk, then runs two configurations, each on a freshly mounted editor
(1040×600 CSS px host) with the same deterministic 5 000-line PostgreSQL document
(`spike/sqlDocument.ts`: DDL, CTEs with window functions, JSONB operators, casts,
`E''` strings, dollar-quoted PL/pgSQL bodies):

- `sql`: PostgreSQL highlighting, keyword completion (`autocompletion()`), line
  numbers, active line, history and bracket matching.
- `vim`: the same plus Vim mode, ahead of every other keymap.

Mounting, a forced full parse, 30 refresh-calibration frames and three settle
frames per edit site are unsampled. At the end of lines 2, 2 500 and 5 000 the
script types 60 characters
(`" AND lower(c.email) ILIKE '%@example.org' OR o.total >= 4200"`), one at a time
through `document.execCommand('insertText')`. This is WebKit's native editing path
(`beforeinput`, DOM mutation, CodeMirror's DOM observer), so it is as close as the
page can get to real typing. It then deletes 20 characters with `keydown` events
(`Backspace`, or Vim normal-mode `x`). Vim enters insert mode with `A` and leaves
with `Escape`; these mode switches are unsampled and verified with `vimModeOf`.
Each keystroke is scheduled 50 ms after the previous paint, so 240 samples per
configuration are taken with no discarded samples.

Per keystroke, using the page clock: `queueMs` is the scheduled time to the dispatch
(it includes main-thread contention such as CodeMirror's background parse slices);
`inputMs` is the dispatch to the first `docChanged` update; `vsyncWaitMs` runs from
that update to the first rAF's frame timestamp (zero if the update lands inside the
frame); and `frameMs` runs from the frame start to a `MessageChannel` task posted
from that rAF. The task runs after the frame's style, layout and paint, so it
includes CodeMirror's measure cycle but not GPU display. The gated
**`keystrokeToPaintMs` = queue + input + frame** excludes only the idle vsync
alignment, because a uniformly random keystroke phase would otherwise add
≈ 8 ms on average and ≈ 16 ms at p95 at 60 Hz, whatever the editor costs.
`wallMs` (dispatch → after paint, including that wait) is reported for every sample.
Gate: nearest-rank p95 ≤ 16 ms per configuration; exactly 16 passes, and there is
no allowance for timer quantisation.
Reports fail closed on: refused insertion; no document update before the paint;
the wrong length change; missing/reordered/extra samples; a wrong site, kind or
delete key; non-finite timings; phases that do not add up; a wrong line count,
mode or PID; or missing timer metadata. Watchdogs are 10 s for module loading,
60 s per configuration and a 130 s report deadline.

WebKit refuses `insertText` until the native window is key. Before each edit site
(unsampled), the page refocuses the editor for up to 300 frames and fails with
"Editor never gained keyboard focus" if the window does not become key. Run it on
an otherwise idle Mac, with the window frontmost and no keyboard or mouse input.
Keystrokes from a person can also land in the editor and cause a
"changed the wrong text" failure.

M4 evidence (Mac mini, Apple M4, macOS 26.5, 60 Hz display, release build, 2026-10-07,
page timer 1.0 ms, `crossOriginIsolated: false`). These are implementer runs, not
maintainer runs. Runs 1–3 were consecutive, using the spike build before the gallery
specimen, editor `aria-label` and focus wait were added; none of those changes
the sampled path. Run 4 includes the `aria-label`. Five later runs of the final
build were rejected with focus errors while the Mac was in interactive use.
None of those runs is counted here.

| Run | sql p50 / p95 / max | vim p50 / p95 / max | >16 ms (sql / vim) | wall p95 (sql / vim) |
| --- | --- | --- | --- | --- |
| 1 | 5 / 11 / 32 | 5 / 9 / 20 | 3 / 3 | 17 / 17 |
| 2 | 4 / 9 / 30 | 6 / 9 / 23 | 2 / 2 | 11 / 10 |
| 3 | 6 / 10 / 31 | 6 / 9 / 26 | 2 / 2 | 10 / 9 |
| 4 | 6 / 9 / 30 | 6 / 10 / 20 | 3 / 3 | 8 / 11 |

All values are in ms. In every run the slowest keystrokes are at the top site, and
most are the `'` that opens or closes a string literal on line 2. That re-tokenises
and re-highlights the rest of the 5 000-line document, so `inputMs` is 18–26 ms.
Every other keystroke stays within one 60 Hz frame. This tail is acceptable for
the spike; M1-21 should keep watching it (for example, quote auto-closing removes
the unbalanced intermediate state). Mac-low is still to be measured.

IME cannot be automated reliably; the manual check is in ADR-0003 and in the S5
PR's "How to try it".


### M0 S3 PostgreSQL decode measurement boundary

`just bench-pg-decode` runs `cargo bench -p openrows-driver-postgres --bench decode_pg`
and then `scripts/bench-pg-decode.py`. The script builds nothing; it reads the latest
Criterion `estimates.json` and `benchmark.json` for each group from the shared Cargo
target. Throughput is element count ÷ the mean's upper 95% confidence bound, a
pessimistic figure; Criterion runs on one thread, so this is per core. Missing,
duplicate or non-finite results fail closed, and `just bench-test` covers the gate logic.

Workloads (`openrows_driver_postgres::fixture`, seed 42, NULL in every seventh cell):
int = 10 alternating `int4`/`int8` columns × 100 000 rows; text = 10 `text` columns
(ASCII plus 0–7 two-byte characters) × 100 000; mixed = 15 types (all supported binary
types including `numeric(18,4)`, unconstrained `numeric` and `jsonb`) plus 5
primitive/text columns × 50 000; wide = 200 columns cycling `int4`/`int8`/`float8`/`text`
× 5 000; first_batch = the mixed schema × 200 rows. Timed work covers frame parsing
(`postgres-protocol` `Message::parse`), decoder construction, cell decoding and every
batch flush. Generating fixtures and copying input frames per iteration are excluded.
The ignored integration test `benchmark_fixture_matches_server_output` proves that
real servers send the same values: the decoded `RecordBatch`es from PostgreSQL equal
those from the synthetic frames.

Local machine: Mac mini, Apple M4, 24 GB, Rust 1.94.0, bench profile (fat LTO, one
codegen unit), Arrow Rust 60. Criterion central estimates: int 3.01 ms (332 M cells/s),
text 15.8 ms (63.3 M), mixed 7.12 ms (140 M), wide 5.91 ms (169 M), first_batch 35.3 µs.
The gate figures in the table use upper bounds. There is no main baseline; these
benchmarks are new.

No per-cell allocation: `tests/allocations.rs` installs `dhat` as the global allocator.
After a 7-row warm-up it decodes 4 992 rows × 11 primitive columns and asserts 0 heap
blocks. The warm-up covers one-off costs: the first frame split shares the read buffer,
and Arrow allocates each column's validity bitmap at its first NULL. Each column also
allocates once per batch when the next builder is pre-sized from the previous batch's
byte length. Arrow buffers move into the finished batch, so they cannot be reused.

First batch end to end: `just test-pg <version>` measures a 200 000-row temp-table scan
(`int4`, `text`, `float8`). Times run from the first `DataRow` to the 200-row batch
reaching the sink, then from `execute()` (describe round trip, execute round trip and
server time). On the M4 with Docker Desktop: PG 12.22 0.142 / 0.867 ms,
PG 17.11 0.141 / 0.884 ms, PG 18.6 0.152 / 1.786 ms. The test asserts the driver part
≤ 5 ms. A `generate_series` source is not suitable here: it materialises all rows
before sending the first (26 ms observed).

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
