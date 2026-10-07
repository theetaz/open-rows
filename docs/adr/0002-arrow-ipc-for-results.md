# 0002 — Apache Arrow for result storage and IPC transport

- Status: Accepted: keep Arrow IPC transport
- Date: 2026-10-05

## Context

Results can be millions of rows. Serialising rows as JSON for the webview is slow, memory-heavy, loses type fidelity (64-bit ints, decimals, binary) and blocks the UI thread while parsing.

## Decision

- Drivers decode directly into Arrow `RecordBatch`es.
- The core keeps results in an Arrow-based **result store** with memory accounting and spill to Arrow IPC files.
- The grid requests **windows (tiles)**; the core returns Arrow IPC bytes via Tauri raw binary responses; a Web Worker decodes them.

## Consequences

- ✅ Zero-copy slicing in Rust; typed arrays in JS; exact types preserved.
- ✅ One format for memory, spill, driver host transport, and Parquet export.
- ✅ Native pass-through for engines that already emit Arrow (ClickHouse `ArrowStream`, DuckDB, ADBC drivers).
- ⚠️ `apache-arrow` JS adds bundle weight (loaded lazily in the worker, not in the initial chunk).
- ⚠️ Arrow types need extension metadata for things like JSON, UUID, geometry and "unknown with original type name".

## Alternatives considered

- **JSON rows:** simplest, too slow and lossy.
- **MessagePack/CBOR rows:** faster than JSON but row-oriented, still per-value decode cost, no compute kernels.
- **Custom columnar format:** smallest JS decoder, but we would own a format and lose ecosystem tools.

## Exit criteria / revisit

If S1 shows a 256×20 mixed tile cannot round-trip in ≤ 3 ms p95 on mac-low, or the JS decoder is too heavy, switch the **transport** (not the storage) to a minimal custom columnar encoding with identical semantics.

## M0 S1 evidence — 2026-10-06

**Decision — Accepted: keep Arrow IPC transport.** Maintainer measurements on
macOS / Mac mini M4, release build, passed the 3 ms total p95 limit in two
`just bench-ipc` runs of 500 tiles each. S1's transport decision is met on this
hardware; mac-low reference hardware remains a revisit point.

| Run | Invoke p50 / p95 / max (ms) | Decode p50 / p95 / max (ms) | Total p50 / p95 / max (ms) | Verdict |
| --- | --- | --- | --- | --- |
| 1 | 0 / 1 / 6 | 0 / 1 / 4 | 0 / 1.00 / 6 | PASS (≤ 3 ms p95) |
| 2 | 0 / 1 / 6 | 0 / 1 / 3 | 0 / 1.00 / 6 | PASS (≤ 3 ms p95) |

The page measured 1.0 ms timer resolution and `crossOriginIsolated: false`.
COOP/COEP headers had no effect in WKWebView through the custom protocol and
were removed from Tauri and Vite configuration. Timer metadata remains in the
report. With 1 ms quantisation, measured p95 ≤ 1 ms implies true p95 < 2 ms;
sub-millisecond percentiles cannot be resolved by this clock. Run 2's mean total
was **0.524 ms** across 500 quantised samples, supporting the decision. Means
average the quantised samples and are useful estimates, not finer timer precision.
The harness now prints and stores the mean for every stage; historical per-stage
means were not supplied.

Revisit on a mac-low hardware run and with real driver data rather than the
synthetic source. If those runs miss the budget, profile invoke and worker
stages before evaluating the minimal custom columnar transport. No fallback
format was built.

Local machine: Mac mini, Apple M4 (10 cores), 24 GB RAM; Rust 1.94.0, release
profile (`opt-level=3`, fat LTO, one codegen unit), Arrow Rust 60.0.0. This is not
the mac-low reference machine. Deterministic seed 42, one million rows, ten types
repeated twice: int32, int64 beyond JS safe-integer range, float64, packed bool,
short Unicode UTF-8, long UTF-8 (128+ bytes), microsecond timestamp, decimal128
(precision 38 / scale 4), binary and JSON UTF-8 with `openrows.json` extension
metadata. Every column contains nulls every seventh row.

Command: `cargo bench -p openrows-results --bench result_window_encode --locked -- --sample-size 50 --measurement-time 3 --warm-up-time 1`.
The benchmark includes lookup, row/column slicing, schema projection and
uncompressed IPC stream encoding at row 543219; source generation is excluded.

| Tile | Criterion central estimate | 95% interval | Budget |
| --- | --- | --- | --- |
| 256×20 | 11.49 µs | 11.415–11.581 µs | ≤ 1000 µs; pass locally |
| 1024×20 | 17.32 µs | 17.239–17.397 µs | context only |
| 256×100 | 46.27 µs | 45.246–48.078 µs | context only |

These benchmarks are new; main has no comparable result encoder baseline.
`just bench-result-encode` runs Criterion and gates the mean's upper 95% confidence
bound at 1 ms using the existing shared Cargo target directory.

`just bench-ui` (also run by `just check`) checks the complete static initial JS
graph, rejects Arrow modules, and gates its gzip sum at 350000 bytes. Arrow and decoding are loaded only by a
worker; the spike client is also a lazy chunk. The original S1 build measured
99731 bytes gzip (99.731 KB) for initial JS, 40232 bytes gzip for the separate
Arrow decoder chunk and 341 bytes gzip for the module-worker entry. The macOS
fix now bundles the decoder into a classic blob worker inside the lazy spike
chunk. The correction build measured 100785 bytes gzip for the initial graph
(1054 bytes / 1.06% above the original S1 measurement, still below 350000);
the lazy spike chunk is about 41.98 KB gzip. These are build-size measurements,
not native IPC timings. `packages/ui/scripts/bundle-size.ts` is the single
initial-graph budget and heavy-module gate, using Vite manifest edges and emitted
chunk-module identities.

### Binary contract and worker representation

Tauri returns `tauri::ipc::Response::new(Vec<u8>)`, which resolves as an
`ArrayBuffer` on its optimized custom-protocol path
([Tauri raw responses](https://v2.tauri.app/develop/calling-rust/)). Tauri 2.12.1's
macOS `postMessage` fallback serializes a raw response as `number[]`
([pinned Tauri source](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.1/crates/tauri/src/ipc/protocol.rs)).
The wrapper preserves `ArrayBuffer` identity and converts validated integer
bytes (0–255) from that fallback into a transferable buffer; malformed values
are rejected. Rust still returns a raw response, not serialized result rows.
The raw command is registered alongside the tauri-specta control-command registry.
Specta exports `WindowRequest` (u32 range arguments), `ResultError` and the spike
report control types. `@openrows/ipc.resultWindow` is the small handwritten
`invoke<unknown>` wrapper that validates the binary return before exposing
`Promise<ArrayBuffer>`; its request type comes from generated Rust bindings.
Rejections use the exported tagged `ResultError` shape; Promise rejection types
are not expressible in TypeScript. The freshness test covers the generated types
and control commands; a Rust test verifies the response body is raw.

The result store clamps counts with saturating arithmetic, allows starts at the
end (empty windows), rejects starts beyond the end, preserves schema metadata
and handles windows crossing stored batches. Array slices share Rust buffers;
IPC encoding copies only the requested values into the stream ([Arrow stream writer](https://arrow.apache.org/rust/arrow_ipc/writer/index.html)).
Compression is disabled. Disk spill, cell truncation and drivers are outside S1.

The worker returns clone-safe typed buffer chunks with validity, offsets, stride,
length, type descriptions and field metadata. Int64 and timestamp buffers remain
64-bit integers; decimal128 remains four 32-bit limbs and its precision/scale is
preserved. Booleans stay packed and text/binary stay bytes plus offsets. There is
no per-cell conversion. Each distinct underlying buffer is transferred once on
the worker reply. The Rust-generated fixture test checks every value and null
across two batches, before and after transferable structured cloning.

### M0 S1 macOS hang correction

Inspection found that the original spike started before the hidden window was
shown and had no init or tile watchdog. WebKit documents visibility-based timer
and process throttling ([WebKit background-page issue](https://bugs.webkit.org/show_bug.cgi?id=263286)).
The new `spike_start` IPC query reveals and focuses the window before worker
loading. Tauri initialization scripts are documented to run before document
scripts, so an ordering failure was not established
([Tauri initialization scripts](https://docs.rs/tauri/2.12.1/tauri/webview/struct.WebviewBuilder.html#method.initialization_script));
the query makes mode detection observable without depending on a global.

Safari 15 supports module workers, so their existence alone is not a confirmed
cause ([WebKit Safari 15](https://webkit.org/blog/11989/new-webkit-features-in-safari-15/)).
The original release custom-protocol loading path was suspected in the hang.
Vite's inline worker and IIFE bundle avoid it entirely
([Vite workers](https://vite.dev/guide/features.html#web-workers),
[Vite worker format](https://vite.dev/config/worker-options#worker-format)); CSP
now admits blob workers/scripts. Application commands are explicitly declared
in the build manifest and granted only to the main window's capability
([Tauri capabilities](https://v2.tauri.app/security/capabilities/)).
Progress, global-error reporting and bounded waits are described in
[the performance measurement boundary](../13-performance.md#m0-s1-macos-failure-diagnostics).
The maintainer subsequently completed both macOS runs above, verifying the
corrected worker path and passing the ≤ 3 ms p95 transport criterion on M4.

### Maintainer WKWebView measurement

After `just build-app`, run `just bench-ipc` on mac-low in a GUI session. It builds
nothing and reuses `bench_common` executable resolution, process attribution,
report persistence and guarded cleanup. An alternate already-built executable
can be selected with `OPENROWS_BENCH_EXECUTABLE`.

The harness sets `OPENROWS_SPIKE=ipc` and a temporary `OPENROWS_SPIKE_REPORT` JSON
path. Only this explicit mode constructs the synthetic source and runs 500
unique deterministic random offsets with a 64-tile LRU client (no cache hits).
Worker creation and lazy library loading finish before sampling; no tile sample
is discarded, including the first decode. Per sample: invoke→bytes received,
page-clock post→typed worker reply (`decodeMs`), and UI request→typed columns returned to the client. Total includes
client scheduling, both worker messages, transferable reply and cache insertion.
The worker's own decode duration excludes message transit and is retained only as
informational `workerDecodeMs`. Invoke, decode and total use the same page clock;
timestamps from different contexts are not subtracted. The page also reports
the minimum positive delta across 100000 consecutive clock reads and its actual
cross-origin isolation flag. The native side atomically writes samples plus p50/p95/max/mean per stage
to the env path and exits. The harness independently validates all samples,
prints the stage table, retains machine/method metadata in `perf-results/ipc-*.json`,
and fails if total nearest-rank p95 exceeds 3 ms. Stage validation permits at most
twice the measured timer resolution of rounding excess; the budget itself gets
no tolerance. Invalid measurements still retain their JSON, raw timings, timer
metadata, diagnostic summaries and an explicit verdict/error. Temporary reports and partial
writes are cleaned on success/failure, timeout and interruption; retained root
identity allows cleanup of WebKit XPCs after the app's self-exit. Synthetic tests
cover statistics, boundary gating, malformed/partial reports, command output
and lifecycle cleanup without launching the application.

### Dependency rationale

All newly added package licences are on docs/22's allowlist and the Rust graph
passes `cargo deny --locked check`:

| Dependency | Purpose / licence | Alternatives and maintenance |
| --- | --- | --- |
| `arrow-array`, `arrow-schema`, `arrow-ipc` 60 | Arrow storage/types and uncompressed stream encoder; Apache-2.0 | Focused arrow-rs crates avoid the larger Arrow umbrella; active Apache release. A custom store/format would duplicate the mandated ecosystem contract. |
| `apache-arrow` 21.2 | Worker IPC decoder; Apache-2.0 | Active Apache JS implementation; a custom decoder is the ADR fallback, deferred until measured need. Lazy worker chunk avoids initial load cost. |
| `criterion` 0.8 (dev only) | Reproducible encoding statistics; Apache-2.0 OR MIT | Existing performance policy requires Criterion; a manual timer would lack its sampling and confidence intervals. |
| `proptest` 1.11 (dev only) | Seeded random window/null/bounds properties; Apache-2.0 OR MIT | Existing testing policy requires property coverage; hand-picked cases alone miss combinations. |

New JS transitives are `flatbuffers` (Apache-2.0), `json-with-bigint` (MIT) and
`@types/node` 25 (MIT); licences were checked in installed package manifests.
`tslib` 2.8.1 was already in the lockfile; no new version was introduced.
Python harnesses use only the standard library. `thiserror`, serde and Tauri/Specta
reuse existing dependencies. No grid, driver or spill dependencies were added.

## M0-S3 evidence — 2026-10-07

The first decision point, "drivers decode directly into Arrow `RecordBatch`es", holds
for PostgreSQL. The prototype `openrows-driver-postgres` appends wire cells straight
into typed Arrow builders, with no intermediate value type. On a Mac mini M4 it
decodes 63–330 M cells/s per core (mean upper bounds; NFR-PERF-05 asks for 1 M), and
the 200-row first batch takes 0.036 ms. Primitive cells cause zero heap allocations
(`dhat` test). UUID, JSON and exact-decimal text carry the extension metadata this ADR
anticipated (`arrow.uuid`, `openrows.json`, `openrows.decimal`). Unknown types are
`Utf8` with `openrows.source_type`. Values without an exact Arrow form (±infinity,
`NaN` in a constrained `numeric`, timestamps after 294247-01-10) currently fail with
a typed error and need a per-value tagged-text design in M1-11. See
[05 §11](../05-database-drivers.md#11-m0-s3-postgresql-streaming-spike) and
[13 §4](../13-performance.md#m0-s3-postgresql-decode-measurement-boundary).
