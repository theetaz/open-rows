# 0002 — Apache Arrow for result storage and IPC transport

- Status: Accepted (to be validated by M0 spike S1)
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
