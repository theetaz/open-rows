# 0010 — One driver host process per hosted session

- Status: Accepted — validation pending (M2-01, M2-04)
- Date: 2026-10-09
- Deciders: OpenRows maintainer (accepted 2026-10-10)
- Requirements: NFR-REL-02, NFR-REL-04, NFR-SEC-01, NFR-PERF-01..03, NFR-PERF-06; FR-APP-07
- Related: [ADR-0002](0002-arrow-ipc-for-results.md), [ADR-0007](0007-session-actor-model.md), [M2-01 and M2-04](../27-delivery-plan.md#5-m2--sql-breadth-and-power-features)

## Context

NFR-REL-02 requires a driver failure to affect only its session while the app stays up. A tokio task boundary contains Rust unwinding, but cannot contain a native segmentation fault, abort or allocator failure. DuckDB embeds a large C++ engine; M2-04 must run it outside the desktop process. Oracle/ODPI-C and later ADBC libraries have the same failure risk ([docs/05 §9](../05-database-drivers.md#9-out-of-process-driver-host)).

Process isolation costs dynamic linking, another runtime, memory, Arrow serialization and packaging work. It must preserve early batches, bounded streaming, cancellation and the existing core-to-webview pipeline. It is a crash boundary, not an OS security sandbox or protection against every form of machine-wide memory exhaustion. Moving a library to another process does not waive [docs/22's licence policy](../22-governance-and-licensing.md#2-dependency-licence-policy): DuckDB is permissively licensed; Oracle Instant Client remains user-installed and never bundled.

### Current implementation, not the older API sketches

| Boundary | Spot-checkable source and implication |
| --- | --- |
| Driver entry point | [`driver.rs`](../../crates/openrows-driver-api/src/driver.rs): `Driver::connect` takes `ConnectRequest` and `ConnectTrace`, returning a `BoxFuture` with `Box<dyn Connection>`. Pure SQL generation methods are synchronous. No `async_trait` or runtime driver plugin loader is required. |
| Session and facets | [`session.rs`](../../crates/openrows-driver-api/src/session.rs): `Session: Send`, `execute_bound`, `cancel_handle`, `ping`, `in_transaction`, consuming `close`; `BoxFuture` is a boxed `Send` future. [`connection.rs`](../../crates/openrows-driver-api/src/connection.rs): `Connection: Session` exposes `ServerSummary` and optional `Catalog`, `Editor`, `Admin`, `BulkLoader`. |
| Connect payload | [`connect.rs`](../../crates/openrows-driver-api/src/connect.rs): `ConnectRequest` holds secret wrappers, file/server endpoints, timeouts, startup SQL and `via: Option<Arc<dyn Dialer>>`. This trait object cannot be serialized. [`file.rs`](../../crates/openrows-driver-api/src/file.rs): named memory databases are explicitly process-shared, not shared across processes. |
| Actors and recovery | [`manager.rs`](../../crates/openrows-core/src/manager.rs): `SessionManager::open`/`open_resilient`, 32-command queues and panic supervision. [`actor.rs`](../../crates/openrows-core/src/actor.rs): cancellation races execution, fires a pre-acquired `CancelHandle` and allows 2 s to drain. [`resilience.rs`](../../crates/openrows-core/src/resilience.rs) reconnects network losses; it must not replay a failed hosted operation. |
| Results and flow control | [`sink.rs`](../../crates/openrows-driver-api/src/sink.rs): synchronous `ResultSink::push_batch`, only `SinkFlow::{Continue, Stop}`. [`results.rs`](../../crates/openrows-core/src/results.rs) maps `Append::Full` to `Stop`. [`store.rs`](../../crates/openrows-results/src/store.rs) appends without waiting for disk and [`shared.rs`](../../crates/openrows-results/src/shared.rs) can exceed its budget while spilling. Pause/Resume in [docs/06 §5](../06-ipc-and-result-pipeline.md#5-back-pressure-across-the-boundary) is a design requirement, not an existing sink variant. |
| Desktop integration | [`connections/drivers.rs`](../../apps/desktop/src-tauri/src/connections/drivers.rs) registers PG, MySQL/MariaDB, SQLite and MSSQL. [`target.rs`](../../apps/desktop/src-tauri/src/connections/target.rs) converts profiles/secrets into driver requests. [`sessions.rs`](../../apps/desktop/src-tauri/src/connections/sessions.rs) opens actors; [`metadata.rs`](../../apps/desktop/src-tauri/src/connections/metadata.rs) owns separate metadata connections. [`error.rs`](../../apps/desktop/src-tauri/src/connections/error.rs) maps core errors to typed `IpcError`. |

At ADR acceptance there was no host or DuckDB crate in the workspace, and [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) has no `bundle.externalBin`. The original host types, messages, numbers and steps below were proposals; this ADR records no implementation or benchmark result.

## Decision

1. Spawn **one `openrows-driver-host` process per live hosted session**, containing exactly one driver connection and its transaction state. Do not pool hosts or put unrelated sessions in one process. Tabs sharing a session share its failure domain; separate sessions have separate hosts.
2. **DuckDB always uses the host** in M2-04. PG, MySQL/MariaDB, SQLite and MSSQL remain in-process: their working cancellation paths and latency need no extra hop. SQLite's C dependency remains a known residual native-crash risk; hosting all native code is outside M2-01. There is no user override to move DuckDB in-process. Future Oracle/ADBC drivers use this boundary after driver-specific review; Tier 3 alone does not mandate hosting.
3. Add a Tauri-free client adapter (`HostedDriver`, `HostedConnection`), protocol crate and host binary. For hosted drivers, the desktop depends on the client and driver metadata/SQL generation, **never on DuckDB or the hosted driver's native client library**. Only the host links the native DuckDB implementation. Native-library-dependent work, including any synchronous generation requiring the library, stays in the host or is refused; ordinary pure SQL generators stay in the parent.
4. Use **two separate local transports**: a duplex control connection and a host-to-parent Arrow data connection. Both are Unix domain sockets on macOS/Linux and named pipes on Windows. Choose length-prefixed **postcard control DTOs and uncompressed Arrow IPC streams** for results. A full data pipe must never prevent Cancel, heartbeat or Close traffic.
5. Keep result storage, spill, window encoding, safety decisions, history and keychain access in the parent. The proxy decodes Arrow batches into the existing sink; the webview still receives only events and requested windows per ADR-0002.
6. Start lazily on connect and exit on close. After failure, fail that session; a **new explicit connect** automatically creates a fresh host. No automatic resurrection of the old session, transaction or in-flight request.

### DuckDB file ownership and session allocation

For embedded DuckDB, one process may read/write a file; multiple processes may open it only when all are read-only ([DuckDB concurrency](https://duckdb.org/docs/current/connect/concurrency)). A host per connection cannot reproduce SQLite's separate writer, metadata, edit and export connections to one file.

M2-04 therefore gives each writable DuckDB file **one shared logical session (`SessionId`) and one host**. A parent registry reserves canonical file identity (including existing-file identity to catch aliases); competing writable opens or read-only opens while a writer is active fail clearly, rather than silently reusing a different session. Multiple all-read-only sessions may have independent hosts. DuckDB's own file lock remains authoritative for other apps and races; no lock stealing or file copying.

Catalog reads, guarded edits and exports for a writable DuckDB file queue through its owning session actor, with a bounded queue and the same safety gates. They do not create hidden connections or additional `SessionId`s inside its host. Catalog refresh waits behind a running query; exports share the session and cannot masquerade as independent background sessions. This is an explicit DuckDB exception to dedicated metadata/background sessions in docs/04, docs/13 and `connections/metadata.rs`; M2-04 must document and expose that queueing before shipping. An edit job uses the owning session only when idle and outside a user transaction, so it cannot commit another tab's transaction.

An in-memory DuckDB profile similarly has one shared session/host; its name is not a cross-process identity. Closing or losing that host loses the in-memory database. M2-04 must explain this before opening it and must not offer transparent recovery of its data. Writable `ATTACH` operations reserve every attached file in the parent before execution; until that reservation and SQL validation exist, writable attachments are refused. These restrictions preserve the one-session failure boundary instead of quietly choosing a multi-session database host.

## Consequences

- A native crash or forced termination cannot directly unwind or corrupt the desktop's address space; unrelated sessions retain their actors, results and cancellation handles.
- The host boundary is reusable by headless callers; no Tauri types enter `crates/`. Results and staged work already owned by the parent survive host loss.
- Local IPC requires serialization/copies; Arrow native arrays still need IPC encoding unless the engine already supplies compatible IPC bytes. No cross-process zero-copy claim is made.
- More processes, startup work and transient buffers consume memory. Limits and aggregate process measurements are required; the existing typical 4 MiB session-buffer target is not silently increased.
- Writable DuckDB loses independent metadata/query/export concurrency. If that trade-off is unacceptable, revise this ADR and NFR-REL-02 interpretation before adopting a database-instance host.
- Release artifacts must include and verify the sidecar for every supported architecture. A missing, incompatible or unsigned helper fails connection with a typed error; it never triggers an in-process fallback or download.

## Alternatives considered

### Isolation and granularity

| Option | Trade-offs and disposition |
| --- | --- |
| In-process + `catch_unwind` only | Cheapest startup and no IPC; complements actor supervision for Rust panics. Cannot catch segfaults, aborts or native OOM termination; rejected for DuckDB and future risky FFI. |
| One host per driver kind | Amortizes runtime/library memory and permits shared embedded database handles. A crash kills every session of that kind; fails NFR-REL-02. Rejected. |
| One host per connection/session | Smallest matching crash domain, simple ownership and targeted kill; adds startup/memory and DuckDB file restrictions. **Selected.** |
| One host per database instance | Supports multiple native DuckDB connections and independent metadata within one writer process. A native crash still kills all its logical sessions. Rejected for this requirement; owner must approve a different failure boundary if reconsidered. |
| WASM sandbox | Offers bounded linear memory and stronger isolation for libraries compiled for it. Does not run existing Oracle/ADBC native clients, changes filesystem/extension/thread support and would require a different DuckDB build. Not the M2 native-driver path. |

### Transport and encoding

| Option | Trade-offs and disposition |
| --- | --- |
| Framed stdin/stdout | Portable, private inherited handles, no endpoint cleanup. A shared stdout mixes large results with responses/heartbeats, and native libraries may print there. Separate inherited data handles could fix it but need extra platform plumbing. Keep stdin for bootstrap only; do not use stdio for the result protocol. |
| Local socket / named pipe | Bounded kernel buffering, independent control/data lanes and async I/O; needs endpoint permissions, rendezvous authentication and cleanup. **Selected**, matching docs/03 and docs/05's transport sketch. No TCP listener. |
| Shared memory batches + control socket | Can reduce bulk copies, but adds handle transfer, allocation quotas, lifetime/acknowledgement races and crash cleanup on three OSes. Defer unless measured pipe/socket throughput misses the budgets after profiling. |
| Postcard control | Small serde-based Rust protocol without generated-code tooling; positional layouts require explicit version/schema agreement. **Selected**; dedicated wire DTOs, not serialized trait objects or existing Rust enums by accident. |
| Bincode control | Similar Rust convenience and throughput; explicit configuration/version discipline still needed, with no clear benefit over the postcard already proposed in docs/05. Not selected. |
| Protobuf control | Better independent-version evolution and language interoperability; introduces schema/codegen maintenance for two lockstep Rust binaries. Reconsider for third-party hosts, not M2-01. |
| JSON/row encodings for results | Easy inspection but type, CPU and allocation overhead violate the Arrow pipeline. Rejected. Arrow IPC remains the data format, with compression disabled for local transport. |

M2-01 proposes `postcard` as its only new serialization dependency (MIT OR Apache-2.0, [upstream licence](https://github.com/jamesmunns/postcard#license), allowed by docs/22); reuse existing serde, thiserror, tokio, Arrow and platform support where possible. OS lifecycle wrappers must be reviewed if they need another crate. M2-04 separately justifies the DuckDB dependency and transitive licences; no dependency is added by this documentation change.

## Protocol sketch

### Rendezvous, handshake and capabilities

The parent creates two random per-host endpoints: Unix sockets in a private 0700 per-run directory (socket access 0600), or Windows pipes with a current-user DACL and remote clients rejected. It launches the fixed bundled helper with only a non-secret driver-kind argument. A bounded inherited stdin bootstrap conveys endpoint names, a random one-use authentication token and parent/host generation identifiers; no profile, password, SQL or path is placed in argv or environment. The child closes stdin after bootstrap. Both lanes authenticate with the token before accepting messages; reject other peers and stop accepting after the child is attached.

`Hello` / `Ready` exchange protocol **1.0**, a wire-schema fingerprint, app/helper build identity, driver kind, generation, frame limits, Arrow IPC compatibility and required protocol features (`cancel`, `credits`, `pause-resume`, `heartbeat`). M2 requires exact protocol/schema/build agreement because the binaries ship together; mismatch fails before credentials or database access. Future rolling compatibility requires an explicit protocol revision, not guessing from serde layouts. Handshake deadline is 2 s; native connection/file open is separate and uses the request's stage deadlines (file open: 10 s).

`Ready` also sends driver `Location`, explicit wire feature IDs and object kinds. The parent's cheap static `Driver::capabilities()` declaration must match the host; unavailable features are never advertised. `Connect` sends a dedicated owned wire request, including zeroizable secret buffers, and returns `ServerSummary`, `ConnectTrace`, actual facets and transaction state. Required capability/facet mismatch is a protocol failure. `Arc<dyn Dialer>` is not sent: M2 hosts support local DuckDB/file requests only and return `Unsupported` for routed requests. Oracle's later host work must define a parent-managed route bridge and TLS verification before claiming `Routes`.

### Framing and operation lifecycle

Each frame starts with a 32-bit little-endian body length, then a fixed versioned header: explicit message kind, generation, 64-bit request ID, result ordinal and sequence. Length is validated before allocation. IDs are monotonic within one generation and never reused; responses echo the request ID. Stale generations, duplicate terminal replies, out-of-order data and unknown required kinds fail the host connection.

Control bodies are postcard DTOs capped at 1 MiB; larger catalog lists and bulk inputs use bounded pages/chunks with the same request ID, never an unbounded collect. Requests exceeding a supported limit are refused with `Unsupported` before dispatch. Normal operation queue capacity is 32, including pending replies; a separate priority lane reserves space for Cancel, Close, flow control and heartbeats. Both lanes have byte limits, not just message counts (4 MiB queued control bytes, 64 KiB reserved priority bytes); priority saturation fails the host rather than dropping cancellation.

| Message family | Meaning |
| --- | --- |
| `Connect`, `Execute`, `ExecuteBound`, `Ping`, facet requests, `Close` | One connection-owning worker serializes normal requests. Facet requests cover the actual `Catalog`/`Editor` methods and optional Admin/BulkLoader when claimed; unsupported facets return `Unsupported`. Bound text values and NULL stay separate from SQL. |
| `Accepted`, `Reply`, `Error` | `Accepted` is receipt, not successful execution. Every dispatched request has exactly one terminal `Reply` or `Error`; failures before receipt are also completed locally. Execute's terminal body includes `ExecSummary`, transaction state and final result sequence. |
| `ResultBegin`, Arrow data, `ResultEnd` | Result ordinal increases for each `begin_result`, including representation changes. Each result is an Arrow IPC stream: schema, dictionaries, ordered record batches, EOS. Data frames carry at most 64 KiB stream fragments. Schema/field extension metadata, exact decimals/int64, nulls and binary survive unchanged. |
| `BatchReady`, `Grant`, `Consumed`, `Pause`, `Resume`, `Stop` | Credit and sink-flow protocol described below. Control messages identify the execution, result ordinal and batch sequence, so a late credit cannot resume another query. |
| `Cancel`, `CancelAck`, `Heartbeat`, `CloseAck` | Out-of-band control handled independently of the connection worker. `CancelAck` only confirms the native interrupt was requested; the execution's terminal response determines completion and reuse. |

Control and data lanes may arrive in different orders. `ResultEnd`/terminal replies name the last data sequence; the proxy waits for those bytes and Arrow EOS, and for accepted batches to reach the sink, before completing the request. Empty results still convey schema and EOS. Only the core `QuerySink` assigns parent `ResultId`s and emits UI query events; the host does not duplicate that event stream. An interrupted stream remains visibly failed/partial, never a successful complete result.

### Backpressure, Pause/Resume and cancellation

The default credit window is **one batch, up to 4 MiB encoded and 4 MiB decoded Arrow memory**. The worker may prepare one bounded batch, publish `BatchReady` with its checked sizes, then waits for `Grant` before sending it or fetching another. The proxy returns `Consumed`/new credit only after the caller's sink accepts the batch. Schema/dictionary state is also charged to the stream's bounded allocation budget. Schema-only/empty results request a grant too; EOS completes an authorized stream without fresh row credit. Credits bound the whole data path, not only its channel length; no background reader accumulates rows while the sink is paused.

Split multi-row batches to fit normal credits. Oversized single rows are not silently truncated: request an exclusive large-batch grant, capped at 64 MiB encoded and 64 MiB decoded, with no other data grant outstanding and a parent-wide reservation. Larger cells/batches fail with a typed `Unsupported` explaining the limit. Validate metadata lengths, nesting, dictionary sizes and decoded allocations before admitting a batch; invalid Arrow is a terminal protocol failure, not trusted merely because the child produced it. Release host-side source/serialization buffers promptly and measure their overlap.

`Pause` revokes future grants; at most the one already granted batch can finish. `Paused` acknowledges its final authorized sequence. `Resume` carries fresh credit; duplicate or late flow messages are idempotent. A pause has no arbitrary query timeout, but Close, Cancel, statement timeout and host health checks remain active. `SinkFlow::Stop` sends `Stop`, discards/drains the outstanding authorized data and waits for execution to finish so the connection can be reused; it is truncation, not a user cancel. A worker stuck stopping is killed after the same 2 s grace.

The synchronous existing `ResultSink` is not changed to block async actors. M2-01 adds a parent result-store admission controller: withhold grants while spill is busy above the global budget; resume on a spill-completion notification once memory is at/below the existing ¾-budget target. If spill fails or the result disappears, send Stop. This requires a new nonblocking store notification/admission hook; current `Append::Continue` alone does not prove there is memory room. For an alternate export sink, return credit after `push` completes. Inside the host, a dedicated blocking-driver worker may wait on credit, while async control I/O stays independent; this is not a new `SinkFlow::Pause` variant in every existing driver.

The proxy obtains a host-backed `CancelHandle` before execution. It sends `Cancel { target_request_id }` on the priority control lane; the host calls the native interrupt without locking the busy connection or queuing behind Execute. Late cancel cannot affect the next request. Delivery target is ≤ 200 ms and DuckDB interruption should finish ≤ 1 s; the core retains its existing 2 s drain deadline. If that deadline expires, the supervisor terminates/reaps this host. A successful drain preserves the session; a forced kill makes it Failed. A statement timeout remains `ErrorKind::Timeout`, distinct from a user `Cancelled` event.

## Failure model and recovery

A parent `HostSupervisor` owns child identity, transport tasks, deadlines and OS cleanup. The child's control runtime emits heartbeats every 1 s, even while its native worker is busy or credit-starved. Three missing intervals trigger a control probe; no reply within 1 s marks the host unresponsive (≤ 4 s while the machine is awake). Timers use a monotonic clock and reset after detected host-machine suspend/resume to avoid killing a healthy child on wake. A live heartbeat proves the control runtime, **not** native-query progress: user queries have no default deadline; explicit Cancel, configured statement timeout and 30 s catalog deadlines detect a stuck worker without misclassifying a legitimate long scan.

| Failure | Detection and typed outcome |
| --- | --- |
| Rust panic, native abort/segfault, unexpected exit | Child exit watcher or EOF immediately poisons the connection; exit status/signal stays in bounded diagnostic metadata, not raw native stderr. Report host failure, fail only its session. |
| Host allocation OOM / memory threshold exceeded | A returned native allocation error is a normal typed statement error if the connection is proven usable. Supervisor-enforced termination is `MemoryLimit`; an unexplained OS kill is `Exited`, not a guessed OOM. |
| Control runtime or process hung | Heartbeat/probe deadline yields `Unresponsive`; terminate the child independently of its runtime. |
| Native worker hung but control alive | Cancel/statement/facet deadline starts the 2 s grace, then terminate. No autonomous timeout is added to an unlimited user query. |
| Corrupt frame, Arrow stream, capability or version mismatch | Reject before unsafe allocation/use, poison both lanes, report `Protocol` or `Incompatible`, reap the child. |
| Parent closes, exits or disappears | Close cancels work and allows at most 2 s total for graceful shutdown before force-kill. Child observes control EOF/parent lifetime and exits; parent exit hooks and supervisor Drop initiate cleanup without blocking the UI thread. |

Propose a **new** typed `DriverError::Host(HostFailure)` with bounded reasons `Spawn`, `Incompatible`, `Exited`, `MemoryLimit`, `Unresponsive`, `Protocol`. Map it to a new core `ErrorKind::DriverHost` and IPC `IpcErrorKind::DriverHost`; handle every exhaustive match explicitly. Current [`DriverError`](../../crates/openrows-driver-api/src/error.rs) has no host variant, and [`CoreError`](../../crates/openrows-core/src/error.rs) has no driver-host kind. Database errors retain `Server { code, message }`, auth/TLS/unsupported/decode mappings retain their meanings, and safe host messages never embed credentials, SQL, row values or native stderr. An IPC/Arrow integrity failure uses the host Protocol reason and discards the session; ordinary valid-driver value decode failures keep their existing mapping.

Host errors must **not** satisfy `is_connection_lost()` or enter `Link::restore`. Add a lifecycle subscription from the proxy/supervisor to the session manager and metadata owner: current actor panic supervision alone cannot notice a host dying while Idle. One completion coordinator marks the session Failed, ends the active result, completes every running/queued caller exactly once and stops the actor; other actors keep running. A metadata-only host fails just its pending catalog request and connection slot; the next explicit catalog request may connect afresh without replaying a partially executed operation.

No background respawn loop. A new connect gets a new generation and connection; spawn/handshake failure has no automatic retry. Three abnormal host exits for one profile/driver within 60 s trip a circuit breaker; subsequent connect returns a clear failure until the interval expires or the user explicitly resets it. Never rerun sent statements, edits, startup commands from a failed partial connect, or COMMIT. A missing commit acknowledgement is an uncertain outcome (`08007` for edit/DDL contracts); do not assert that a write or DuckDB file change was rolled back. Keep staged changes and accepted partial result batches in the parent, but require refresh/reconciliation before reusing them for edits. Durable DuckDB-file recovery belongs to DuckDB; volatile in-memory contents cannot be restored.

### Memory containment boundary

Accepted defaults: DuckDB engine `memory_limit` 128 MiB; supervisor threshold 256 MiB per host and a parent-wide 512 MiB active-host reservation (at most two default-sized hosts, fewer if an extra large-batch reservation is needed). Charge all hosted operations and exclusive large-batch grants to that reservation; refuse a new host or large grant when its reservation will not fit. These host limits apply in addition to docs/04's per-profile session limit. Sample child memory every 100 ms independently of its heartbeat. Engine settings are not a hard process-memory cap; native allocations and IPC buffers also count. User SQL cannot raise the supervisor threshold.

Use Windows Job Objects for enforceable per-process/aggregate limits and kill-on-parent-close; Linux cgroup v2 caps when delegation is available, otherwise monitoring; macOS uses parent monitoring and force-kill. OS wrappers stay in the lifecycle layer with reviewed platform APIs. These are proposed limits, not measured idle footprints. A monitor can overshoot between samples; **no portable hard guarantee against the OS killing the app during machine-wide OOM is claimed**. M2-01 must verify controlled memory-failure containment without exhausting the developer's machine. The owner accepted this monitoring-based containment as sufficient for NFR-REL-02 (decision 2).

## Security and distribution

The helper runs as the same unprivileged user, never elevated. Without an OS sandbox it can technically read/write everything that user can and access the network; IPC validation is not an OS access-control boundary. Its intended authority is only the chosen database/attachments, its private DuckDB temporary directory and explicitly approved driver resources. It does not open the keychain, app history/profile store or invoke arbitrary executables. DuckDB SQL can access files, so read-only database mode is not a filesystem sandbox; safe-mode classification and engine-specific restrictions must stay effective.

Pass secrets only after the authenticated handshake, through the control pipe/socket in zeroizable owned buffers; clear bootstrap tokens and request copies after use. Start with a cleared environment and an explicit non-secret locale/platform allowlist; no inherited credential or loader-injection variables. Launch by resolved absolute bundled path, never PATH or a shell. Drain stdout/stderr into bounded discard sinks; host diagnostics use typed control records with docs/14 field discipline, not arbitrary native output. No payload/credential logging or core dumps in routine operation. DuckDB persistent secrets and automatic extension install/load are disabled for M2-04; any later extension/network policy requires explicit review. Optional OS sandboxing is later work (macOS sandbox profiles, Linux namespaces/seccomp, Windows restricted tokens); it must preserve approved file access and is not an M2-01 promise.

Ship the binary as the Tauri 2 sidecar `openrows-driver-host`, configured by `bundle.externalBin = ["binaries/openrows-driver-host"]` in `apps/desktop/src-tauri/tauri.conf.json`. Build staging supplies `binaries/openrows-driver-host-<target-triple>` (`.exe` on Windows), as required by [Tauri's sidecar convention](https://v2.tauri.app/develop/sidecar/). A Rust launcher resolves the installed helper and uses raw process/transport handles; bundling as a sidecar does not require granting webview shell execution or adding the shell plugin. Development/test callers inject an explicit built-helper path; installed builds do not accept arbitrary helper-path overrides.

| Platform | Packaging and verification |
| --- | --- |
| macOS | Sign nested helper/native libraries before signing the outer app, using the same Developer ID team and hardened runtime. Verify the helper's designated identity; notarize/staple the final artifact per [docs/16](../16-platform-and-distribution.md#3-macos-specifics) and [Tauri signing](https://v2.tauri.app/distribute/sign/macos/). DuckDB needs neither JIT nor disabled library validation. Start with per-architecture arm64/x86_64 artifacts; a later universal app must embed a universal helper too. Oracle's future user-installed unsigned libraries require separately reviewed helper entitlements; never relax the main app's entitlements. |
| Windows | Bundle the matching x64/arm64 helper and its permitted native dependencies in MSI/NSIS; Authenticode-sign helper and app, verify installed identity, no console window or admin requirement. Use restricted local pipe ACLs and Job Object cleanup. |
| Linux | Include matching executable/dependencies with executable permissions in AppImage/deb/rpm; use the verified package/updater artifact, no user-installed DuckDB requirement. Flatpak needs its own file-access/packaging verification at M6; no sandbox bypass. |

App and helper update together under the existing signed artifact/update chain; do not overwrite an active host or replace the helper independently. A signature/identity/protocol mismatch is a connection failure. Count the helper and native libraries against docs/16's ≤ 40 MB installer target: DuckDB may exceed it, but this ADR does not approve a larger download. M2-04 must measure size and seek an explicit budget revision if needed. Retain SBOM/notices; an out-of-process library still ships with the product.

## Performance budget and benchmarks

Budgets are proposals for implementation validation on **mac-low**, release builds, with win-mid/linux-mid repeats before those platforms ship. Existing M4 encoder/IPC measurements in ADR-0002 are context, not a host baseline. Driver registration must not spawn the helper or synchronously read credentials during app startup.

| Metric / benchmark M2-01 must add | Boundary and target |
| --- | --- |
| `host_startup` | 30 fresh process launches: parent spawn → authenticated Ready, excluding database open. Cold p95 ≤ 150 ms, warm p95 ≤ 75 ms; retain first launch and all raw samples. M2-04 repeats with the actual DuckDB-linked binary, including its dynamic linking. |
| `host_batch_transport` | One-batch synthetic operations using first 200×20 and later ~64 Ki-cell batch shapes (ints, decimals, text, binary, nulls); parent monotonic Execute → sink times for identical in-process/host-backed producers. Paired incremental p95 overhead ≤ 1 ms per normal ≤ 4 MiB batch; includes host Arrow encode, transfer, proxy decode/admission, excludes deliberate pause and database execution. Retain both timings; no subtraction of clocks across processes. Large 64 MiB batches reported separately, target ≥ 200 MiB/s sustained. |
| `host_stream_throughput` | 1 M rows, short/wide/mixed data, identical batch policy and sink: host-backed end-to-end cells/s ≥ 80% of in-process baseline; include memory/copies and cancellation while a slow sink applies pressure. No result JSON or per-cell proxy conversion. |
| `host_backpressure_memory` | Slow/stopped sink, full/failed spill, two default-sized hosts, rejected excess-host admission and an exclusive large row: one outstanding grant, ≤ 4 MiB typical source driver buffers, measured transient IPC allocations target ≤ 16 MiB per normal batch across both processes; no growth with result length. Report oversized batch peaks separately. Retained result-store overhead still ≤ 1.5× Arrow bytes (NFR-PERF-06); transport/engine costs are separately counted, never hidden in that ratio. |
| Native startup / first paint / memory | Existing startup targets remain cold ≤ 800 ms, warm ≤ 400 ms. Host hop fits inside docs/13's ≤ 25 ms app-overhead budget: ≤ 1 ms hosting inside the existing 5 ms first-batch stage, not a new allowance. Preserve the ≤ 3 ms tile transport and ≤ 1 ms window encoding gates; NFR-PERF-03 remains ≤ 50 ms from first batch at core to paint. Measure one connected DuckDB/tab including **all** helper processes against ≤ 200 MB idle footprint; lazy launch alone does not prove the connected workload meets it. |

Use the existing Criterion approach for codecs/encoding and a parent-timed subprocess harness for p50/p95/max/throughput and memory attribution. Pair real process transfer with an in-process baseline in the same run; report hardware, versions, sample counts, batch sizes, dictionary/extension metadata and actual timer resolution. M2-04 adds native first-batch/cancel/file workloads and installer size. Profile misses before proposing shared memory or accepting budget changes. Feature-first mode in docs/25 defers regression comparisons to milestone checkpoints; these benchmark fixtures still belong to implementation, with reference-hardware acceptance at the M2 checkpoint. This documentation task runs no benchmark and supplies no invented numbers.

## Testing and implementation plan

The existing [`openrows-driver-conformance`](../../crates/openrows-driver-conformance/src/lib.rs) accepts `Target { driver: Box<dyn Driver>, request, schema }` and `conformance_tests!`. Run **the same suite/assertions** against a host-backed SQLite test adapter first; the existing SQLite fixture/dialect is usable without modifying conformance tests or shipping SQLite hosting. Test-only drivers and crash hooks are excluded from the production helper. M2-04 adds DuckDB's fixture/dialect expectations without weakening group assertions; claimed facets determine applicable groups. The current suite does not cover 1 M-row bounds, Pause/Resume or all crash semantics, so the host needs additional tests.

Each row below is a separate reviewable implementation change after owner acceptance; none is done by this ADR.

| Step | Scope | Acceptance criteria / evidence required |
| --- | --- | --- |
| M2-01.1 — protocol | Wire DTOs, postcard codec, two-lane framing, version/capability agreement and owned connect conversion. | Round-trip control and Arrow fixtures preserve all metadata/types; fragmented/truncated/oversized frames, dictionary expansion, unknown kinds, stale IDs and incompatible builds fail boundedly before credentials/unsafe allocation. Routed `Dialer` requests return `Unsupported`. Codec benchmarks exist. |
| M2-01.2 — supervisor | Host binary, independent control runtime and dedicated connection worker, platform launcher/cleanup, health and memory policy. | Inject native abort/segfault, Rust panic, simulated allocation failure, capped child memory growth, missing heartbeat and parent death; only the identified child is terminated/reaped, endpoints/temp dirs removed, no orphan. Verify OS caps where available and record monitoring-only fallback. No global-OOM stress test. Startup harness exists on all three OSes. |
| M2-01.3 — adapter and core | `HostedDriver`/`HostedConnection`, actual facets, lifecycle observer, typed host errors, one terminal coordinator, credits/store notifications, Cancel/Pause/Resume/Stop. | Unchanged conformance suite passes via host-backed SQLite; core tests show idle and busy host loss, queued callers, catalog/edit errors, partial results and exactly one terminal outcome. Crash A while independent hosted B and in-process C keep querying; UI events remain responsive. Cancel delivered ≤ 200 ms when data is paused/full; non-cooperative worker gone after 2 s grace. All frame/memory limits hold; throughput/backpressure harnesses exist. |
| M2-01.4 — packaging and acceptance | `externalBin`, staging/resolution, signed artifact integration and proposed-policy updates to docs/03, /04, /05, /06, /12, /13, /14, /16, /27. | Installed artifact finds its helper without PATH; wrong/missing helper fails typed, never falls back. macOS Developer ID/notarization smoke when signing prerequisites exist; Windows/Linux package launch/close smoke at their release milestone. Record real commands/results and reference-hardware numbers; unavailable signing/hardware stays a named gap, not a passed gate. Owner resolves release blockers before shipping. |
| M2-04.1 — native DuckDB | Host-only native dependency, one writable-file/shared-memory session policy, registry reservations and file-mode errors; no automatic extensions/persistent secrets. | Desktop dependency graph contains no DuckDB library; same writable file via aliases cannot gain another host; all-read-only hosts work; contention from another app fails clearly; memory databases share only within the sole session and report loss on host exit. Explicitly verify queueing of metadata/export and refusal of writable attachments until reservation support exists. |
| M2-04.2 — Tier 2 behavior | File connect/create/read-only, streaming/binding, catalog, guarded edits/transactions and core-driven export through the proxy. | Applicable unchanged conformance assertions pass with DuckDB fixtures; every claimed feature/facet agrees. Long query interrupts ≤ 1 s and next query succeeds; controlled native/OOM/worker failure leaves another session and UI working. Commit/ack loss is uncertain, never replayed; staged edits persist. First-batch/type fidelity, slow-spill Pause/Resume and large-cell limits have native integration evidence. |
| M2-04.3 — owner acceptance | Native app journeys, signed DuckDB artifacts, aggregate memory/size and M2 checkpoint budgets. | Open a DuckDB file, browse/query/edit/export, cancel, force host loss and reconnect explicitly; PG/MySQL/SQLite/MSSQL continue. Report real host startup/batch/throughput/memory numbers vs budgets and ≤ 40 MB installer result; misses require an approved revision. Update docs/26/27 with implemented evidence only after this work passes. |

Crash tests use isolated disposable child processes and synthetic/local fixtures; distinguish a declared memory-limit kill from unexplained OS exit. Add races for Cancel versus completion/new request, Close versus hung worker, Pause versus outstanding grant, heartbeat under data saturation, malformed Arrow, metadata-only host death, failed handshake secret handling, repeated crashes/circuit breaker and old-generation replies after reconnect. Check argv/environment/logs for an obvious test-only secret; ensure no payload reaches diagnostics. Tests clean up their own child processes and files even after timeout/interruption.

## Owner decisions — 2026-10-10

The owner accepted every proposed default below without changes. They are binding for M2-01 and M2-04.


1. **One writable DuckDB session — accepted.** Metadata, exports and edits on a writable file queue on its single hosted session. Independent connections to that file would require revising granularity and NFR-REL-02 first.
2. **Monitoring-based memory containment on macOS/Linux — accepted** as sufficient for NFR-REL-02. Defaults: 128 MiB engine, 256 MiB host, 512 MiB aggregate, 100 ms monitor. Machine-wide OOM is outside the guarantee.
3. **Typed host error, terminal Failed state and reconnect — accepted.** A new explicit connect creates a fresh host; three exits in 60 s open the circuit breaker; sent work is never replayed.
4. **Flow-control limits — accepted.** One-batch 4 MiB credit window, exclusive 64 MiB large-row ceiling (larger rows fail explicitly), 1 ms per-batch and 150 ms cold-host targets, subject to reference-hardware measurement.
5. **Budgets kept — accepted.** The ≤ 200 MB connected idle footprint and ≤ 40 MB installer budgets stay; a DuckDB miss blocks release until an explicit budget or design revision. The maintainer owns the signing and reference-machine checkpoint.
6. **Restricted DuckDB extensions and persistent secrets; host-local, non-durable memory databases — accepted.** Routed Oracle/ADBC support, stronger OS sandboxing and universal macOS artifacts remain separate reviewed work.

## Validation / exit criteria

The owner answered the policy questions above on 2026-10-10. M2-01 establishes the protocol, isolated lifecycle, unchanged conformance path, failure/cancel/backpressure tests and benchmark harnesses; M2-04 validates those choices with the actual native engine and installed artifact. A multi-session crash domain, automatic write replay, loss of control while paused, unbounded host/parent allocation or any in-process DuckDB linkage invalidates the design. Revisit transport only after profiling a budget miss; revisit granularity if writable-file concurrency is required.

## Validation / evidence — 2026-10-09

Design proposed for M2-01 and accepted by the owner on 2026-10-10; current source pointers and official sidecar/DuckDB constraints were inspected. No Rust/TypeScript, host executable, IPC integration, crash test, signed package or performance measurement is implemented by this change. The M2-01/M2-04 evidence above remains pending. Existing accepted ADRs remain in force.

## Validation / evidence — 2026-10-10 (M2-01a)

M2-01.1–.2 are implemented in `openrows-host-protocol`, `openrows-host-client` and `openrows-driver-host`. The helper's only compiled driver is SQLite; the desktop has no host-crate/DuckDB dependency and no hosted UI path. Catalog/editor/admin/bulk wire placeholders return Unsupported and are not advertised. Core adapter/conformance/store admission/terminal coordination and installed packaging remain M2-01.3–.4.

Disposable-process tests cover abort, deliberate SIGSEGV, Rust worker panic, recoverable and fatal simulated allocation failure, controlled 272 MiB allocation exceeding the 256 MiB RSS threshold, a missing heartbeat, noncooperative Cancel/Close, Drop, and actual parent death. They assert targeted PID disappearance/reap and endpoint cleanup; a second live host queries after native/memory failure. Framing/property tests exercise caps, malformed/truncated/fragmented/unknown frames, stale identities, incompatible builds/features, dictionary expansion, exact Arrow values and metadata. Handshake tests inspect literal argv/environment/command diagnostics and captured stdout/stderr for the obvious test-only token.

The measured platform is macOS 26.5, Apple M4 (Mac16,10), 24 GiB RAM, Rust 1.94.0; builds use four jobs and nextest four test threads. macOS containment is explicitly monitoring-only plus targeted force-kill. Windows named-pipe/Job Object and Linux delegated-cgroup/fallback implementations are behind cfg and have not been compiled or run on their target platforms here. No global OOM stress, signing, installed package, DuckDB, core proxy or GUI verification is claimed.

Opt-in release subprocess benchmarks (`OPENROWS_BENCH=1`, parent `Instant`, measured timer resolution 41 ns) retained raw CSVs under `perf-results/`. Fresh launches (30, first retained) measured p50/p95/max 4.229/8.204/8.577 ms against the 150 ms fresh-host target; subsequent 29 launches measured warm p95 7.261 ms against 75 ms. OS file caches were not evicted, so this does not certify filesystem-cache-cold startup.

| Paired producer → sink benchmark | In-process p95 | Hosted p95 | Paired incremental p95 | Budget |
| --- | --- | --- | --- | --- |
| 200×20 mixed cells, 30 samples | 0.067 ms | 0.443 ms | 0.394 ms | ≤ 1 ms |
| 3277×20 mixed cells, 30 samples | 0.302 ms | 1.253 ms | 0.951 ms | ≤ 1 ms |
| Exclusive 63 MiB single-row payload (within 64 MiB total ceiling), 5 samples | Raw paired timings retained | 48.338 ms | 1661.910 MiB/s sustained | ≥ 200 MiB/s |

Before optimization, the larger normal shape measured 2.431 ms incremental p95. A 2 s macOS `sample` profile of the owned helper showed repeated fragment-channel waits (300 samples) and serialization (31 samples); the default local stream send/receive spaces were 8192 bytes. Moving each authorized segment once to its independent writer, combining the frame prefix/header write, and configuring bounded 512 KiB data-socket buffers reduced the measured overhead without shared memory or changing credit limits. Splitting prepares one part at a time. Socket kernel buffers are separate bounded overhead; whole-process/copy attribution belongs to the M2-01.3 backpressure-memory harness.

Criterion control codec: encode 85.039 ns (84.262–85.998 ns interval), decode 135.25 ns (133.83–137.08 ns), 100 samples each. These are component timings with a Hello fixture, not end-to-end transport latency or a main-branch regression comparison.

The final raw samples are `perf-results/host_startup-79284.csv`, `perf-results/host_batch_transport-79272.csv` and `perf-results/host_large_transport-79272.csv`. The normal transport shapes encoded 55680 and 859200 bytes respectively; benchmark fixtures mix int64, decimal128, UTF-8, binary and nulls without dictionaries or extension metadata. Separate protocol fixtures cover dictionary and extension fidelity.

The full Rust gate passed `1245 tests run: 1245 passed, 196 skipped` in 42.623 s, including all 34 new protocol/client/host tests. Skipped tests are the existing external-database suites. The queued-write cancellation test proves an acknowledged queued cancel prevents the write from reaching SQLite. One earlier concurrent debug run had four 2 s handshake deadline failures; the repeated run passed. Deadlines remain the accepted 2 s, without automatic retries in the supervisor.

The focused paused/stalled-reader test passed (`1 test run: 1 passed, 14 skipped`): CancelAck was 0.283 ms with credit paused and 0.204 ms with the data reader stalled, both within the 200 ms target; the test uses a 500 ms CI assertion margin. Independent heartbeats and Close remain live in both cases. Formatting, clippy, dependency/advisory/licence and hygiene gates passed. UI token checks passed; initial JS is 289287 gzip bytes against 350000, and component CSS is 6187 against 15000.

An offline Windows cross-check (`cargo check -p openrows-host-client -p openrows-driver-host --target x86_64-pc-windows-msvc --offline --locked`) failed with `E0463: can't find crate for core`: this machine has no Windows Rust target. This is an open platform compile/validation gate, not evidence that the Windows implementation compiles. Reference-machine acceptance and M2-01.3 throughput/backpressure-memory harnesses remain milestone work.

Frontend eslint, format, typecheck and knip gates passed; `pnpm test --maxWorkers=2` passed 226 test files / 2680 tests in 165.70 s. Generated IPC freshness passed (`1 test run: 1 passed, 230 skipped`). The default-feature release helper built successfully in 1m 39s; none of the nine crash/fixture hook markers occurs in that binary. Normal dependency graphs confirm no host/DuckDB dependency in the desktop and no Tauri/native database dependency in protocol/client. All new Rust modules remain below 300 lines. Build artifacts and benchmark CSVs are retained locally for the maintainer; disposable tests leave no owned hosts or rendezvous directories.


## Validation / evidence — 2026-10-10 (M2-01a, review round 2)

The Linux reproduction failed 10 of 15 host tests with `MemoryLimit` in 0.04 s. Running the unchanged tests with one libtest thread passed all 15: unrelated parallel tests were sharing the intentional process-wide 512 MiB reservation. Host-reserving tests now hold an asynchronous test-only guard for their lifetime within one libtest process. The production 256 MiB host / 512 MiB parent-wide admission policy is unchanged, and a policy assertion verifies separate default budgets share the same parent reservation. Nextest still runs four independent test processes concurrently.

Host, handshake and parent-death fixtures now create private rendezvous directories under `/tmp` on Unix (the system temporary root on Windows), keeping Unix socket paths short and leaving the source tree untouched. The production endpoint root remains caller-supplied. RSS regression tests cover a missing process, a zombie status without VmRSS, malformed/overflowing values, permission/read failures, the exact threshold and a real over-threshold sample. Only the latter triggers monitored `MemoryLimit`; unreadable samples leave exit/heartbeat detection responsible for lifecycle failure.

`cargo test --locked -p openrows-host-protocol -p openrows-host-client -p openrows-driver-host` passed in Linux Docker (`rust:1.94.0-bookworm`, aarch64, four CPUs/build jobs), copying the source to `/work` and using `/target/work` in `orows-linux-target`. Both root and UID/GID 1000 passed all 36 tests: 15 host, two client RSS/monitor unit tests, three client policy tests and 16 protocol tests; no failures or ignored tests. The host result lines were:

```text
root:    test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.21s
nonroot: test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.22s
```

Both reported `Linux containment: monitoring-only fallback (cgroup v2 not delegated)` with the default read-only cgroup mount. An additional root run with a writable cgroup mount also passed all 36 tests (host tests 8.27 s); enabling the memory controller was refused (`sh: 1: printf: printf: I/O error`), and the supervisor correctly reported the fallback. These runs verify unavailable-delegation handling and controlled RSS containment, not a working delegated kernel cap or installed Linux package.

The first full macOS gate exposed three two-second handshake timeouts during concurrent debug startup (`404/1263 tests run: 401 passed, 3 failed, 196 skipped`). A closed-stdin launch with no IPC reproduced 1.66–1.78 s startup latency. macOS test fixtures now prepare the debug image once per test process with a bounded ten-second closed-stdin probe: it receives no bootstrap, opens no endpoints/database and is reaped, with targeted kill/reap on timeout. This setup cost is separate from the functional protocol tests. Actual supervisor handshakes retain the two-second deadline and no retries; production launch and the opt-in cold/startup benchmarks do not use this preparation. The setup does not certify cold debug-launch latency under load.

Three consecutive final macOS host-test runs with `NEXTEST_TEST_THREADS=4 CARGO_BUILD_JOBS=4`, `nice -n 10`, and `cargo nextest run --locked -p openrows-driver-host --test host --success-output immediate` passed `15 tests run: 15 passed, 0 skipped` in 5.983 / 5.890 / 5.724 s. CancelAck (paused / stalled reader) measured 0.298 / 0.239 ms, 0.253 / 0.221 ms and 0.196 / 0.215 ms against 200 ms. Linux clippy for all targets of the three crates passed with `-D warnings` (exit 0, 0.44 s).

The installed Windows Rust target advanced `cargo check --locked --target x86_64-pc-windows-msvc -p openrows-host-protocol -p openrows-host-client` to the `ring v0.17.14` C build. It failed with `fatal error: 'assert.h' file not found` and `failed to run custom build command for ring v0.17.14`; `VCINSTALLDIR = None`. MSVC C headers/toolchain are unavailable, so this supersedes the earlier missing-Rust-target blocker but does not validate the Windows cfg implementation. No workaround or production dependency change was introduced.

No subprocess/codec benchmark was rerun for these test-isolation and regression-coverage changes. The prior retained CSVs were re-read: fresh/warm startup p95 8.204 / 7.261 ms, paired incremental transport p95 0.394 / 0.951 ms and exclusive-row throughput 1661.910 MiB/s. Those remain round-1 measurements with the hardware/cache limitations above.


The final macOS Rust run passed `1263 tests run: 1263 passed, 196 skipped` in 23.840 s, including the host/client/protocol tests; skips remain the existing external-database suites. Formatting, workspace clippy (`-D warnings`), advisories/licences/bans/sources, unused-dependency and hygiene checks passed. Generated IPC was refreshed through `just gen-ipc`. UI tokens are fresh and component colours use tokens; `just bench-ui` measured initial JS 290860 gzip bytes / 350000 and tokens/component CSS 6187 / 15000. Benchmark-script unit tests passed 89 tests. Shell-tooling tests initially failed under the shell-selected Xcode Python 3.9 (`TypeError: unsupported operand type(s) for |: 'type' and 'NoneType'`); rerunning `just script-test` with `/opt/homebrew/bin` first in PATH selected installed Python 3.14.7 and passed all six tests. Frontend eslint, formatting and typechecks passed. No repository change was needed for the Python selection.


`pnpm test --maxWorkers=2` passed `Test Files 227 passed (227)` / `Tests 2685 passed (2685)` in 90.60 s. Knip passed, and `just ipc-check` passed `1 test run: 1 passed, 240 skipped` in 0.153 s. Normal dependency graphs again verify no host/DuckDB dependency in the desktop and no Tauri/native database dependency in protocol/client. Round 2 adds no dependency. Temporary verification logs and the disposable image probe were removed; created containers were removed, while `orows-linux-target`, the existing build cache and prior benchmark CSVs are retained. PID/reap and endpoint-cleanup assertions passed; there are no owned hosts or source-tree rendezvous directories left. Core adapter, packaging, delegated-kernel-cap validation and Windows validation remain the named follow-up scope above.


## Validation / evidence — 2026-10-10, M2-01b

`HostedDriver`/`HostedConnection` now implement ADR step M2-01.3 in the Tauri-free
client. Catalog and guarded edits use dedicated DTOs on the serial native worker;
large catalog replies use one acknowledged 64 KiB fragment at a time, capped at
16 MiB and 65536 top-level entries (4096 per nested collection). Synchronous
decode also limits aggregate collection allocations to a conservative 32 MiB;
text bytes remain bounded by the encoded ceiling. A 5000-table catalog exceeding
1 MiB passes. Unsupported bulk/admin facets are not advertised.
The default helper registry has no native SQLite dependency; test fixtures enable
`test-sqlite`/`test-hooks`. No desktop production driver uses the adapter.

Nonblocking store admission pauses above budget and resumes at the existing
¾-budget spill target; failed spill/released results Stop and drain. Arrow arrays
reach the sink directly. Idle lifecycle observation fails only the owning actor
or metadata slot. Busy failure ends partial results before completing queued
callers; hosted sessions never enter network restoration or idle suspension.
Lost edit/DDL acknowledgement is `08007`, without a rollback or replay claim.
A disposable file test confirms an acknowledged-lost commit was applied exactly
once and is visible after a new explicit connect.

The targeted final-source macOS nextest run passed `199 tests run: 199 passed, 33 skipped`
in 8.544 s. The separate unchanged conformance run passed
`10 tests run: 10 passed, 0 skipped` in 1.993 s (nine applicable groups; process
administration is unsupported). Host A loss leaves
hosted B and native C querying. Busy loss preserves 200 accepted rows, one result
end and one typed terminal error for each of eight queued callers. Paused adapter
cancel/drain was 0.184 ms; raw CancelAck was 0.167 ms paused and 0.199 ms with the
data reader stalled. A non-cooperative worker was reaped after 2001.080 ms.
`is_connection_lost()` remains false and a resilient hosted session's connect
recipe runs once, without startup/query replay.

Repeated Pause from a resume future cannot issue credit; the default paused
future waits without spinning. Failure signaling preserves the first reason
before EOF/cleanup, even when the supervisor queue is full. Refused exclusive
credit permanently fails only that owner with `MemoryLimit`; the other host
continues querying. Native connect failure preserves its completed stages/TLS
trace and closes normally, without counting a startup refusal as a host crash.
Client timeout remains Timeout for the active caller while non-cooperative host
loss fails queued callers and the session with DriverHost.

The final macOS Rust refresh passed `1307 tests run: 1307 passed, 196 skipped`
in 27.881 s; skips are existing external-database suites. Formatting, workspace
clippy (`-D warnings`), advisories/licences/bans/sources, unused dependencies and
hygiene passed. `just gen-ipc` made no bindings change; `just ipc-check` passed
`1 test run: 1 passed, 248 skipped` in 0.181 s. Frontend eslint, formatting,
typecheck and knip passed. UI tests passed 230 files / 2701 tests in 96.35 s;
initial JS was 301302 gzip bytes / 350000, and token/component CSS was 6294 /
15000. Benchmark-script tests passed 89 tests; shell-tooling tests passed six
using the installed Homebrew Python. `OPENROWS_REQUIRE_DOCKER=1 just test-pg`
passed `78 tests run: 78 passed, 0 skipped` in 15.032 s on PostgreSQL 17.11 with
TLS, including native reconnect, timeout, cancellation and conformance coverage.

Linux final-source validation copied the worktree into `/work` in
`rust:1.94.0-bookworm` (aarch64), with Docker `--init --cpus 4`, four Cargo jobs
and four libtest threads. The five host/core/result crates passed 200 tests,
with 33 existing database tests ignored; desktop `--lib` passed another 248.
Representative real result lines were:

```text
host conformance: test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
host lifecycle:  test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.84s
desktop:         test result: ok. 248 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
```

Linux paused adapter cancel/drain was 0.228 ms; raw CancelAck was 0.199 / 0.235 ms
(paused / stalled reader), and non-cooperative reap was 2003.738 ms. Both new
admission regressions, metadata-slot invalidation and the desktop error mapping
passed. The initial container without `--init` left the parent-death fixture's
orphan as a PID-1 zombie; using the init reaper passed the unchanged assertion.
Containment reported the accepted monitoring-only fallback because cgroup v2
was not delegated. Containers were removed; `orows-linux-target` is retained.

Release benchmarks used Apple M4 (Mac16,10), 24 GiB RAM, macOS 26.5 arm64,
Rust 1.94.0, Arrow 60, `nice -n 10`, four Cargo jobs and two runtime workers.
Minimum observed parent `Instant` resolution was 41 ns. Each throughput row is
one native/hosted pair with identical default BatchPolicy and counting sink,
1 M rows, native first; these are local samples, not confidence intervals or
mac-low reference-machine acceptance. SQL fixtures include UTF-8/binary/nulls,
without dictionaries/extensions (those remain covered by protocol tests).

| Shape | Columns | Native cells/s | Hosted cells/s | Native ms | Hosted ms | Hosted/native (budget ≥ 0.80) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Short | 1 | 7452261 | 8386242 | 134.187 | 119.243 | 1.125 |
| Wide | 20 | 41320841 | 41848428 | 484.017 | 477.915 | 1.013 |
| Mixed | 5 | 17265241 | 19192569 | 289.599 | 260.518 | 1.112 |

These final-source measurements supersede earlier development samples; each
fixture ran once after the final adapter fixes. The backpressure harness admits
the first schema before inspecting store pressure; an initial harness run
stopped before that schema and produced no spill rows, which was corrected
before recording spill evidence. It sums parent + owned-child RSS at 5 ms
intervals (32 samples while paused), including native-engine/allocator costs and
possibly double-counted shared pages.
This is a conservative sampled process delta, not exact allocation attribution
or macOS physical footprint. No reader accumulates data during Pause: both result
lengths below accepted only 200 rows, with stable final/peak paused RSS.

| Fixture | Baseline RSS | Peak/final paused RSS | Delta (normal target ≤ 16 MiB) | Cancel/drain ms |
| --- | ---: | ---: | ---: | ---: |
| 100000 rows | 15532032 | 28590080 | 13058048 | 0.087 |
| 1000000 rows | 16007168 | 30244864 | 14237696 | 0.116 |

Both normal cases had one outstanding grant, 2112 encoded / 2644 decoded bytes
at first-batch admission. A real 1 MiB spill store peaked at 1066912 Arrow bytes
(one admitted batch beyond budget); successful spill retained 1000000 rows,
failed spill stopped/truncated at 131272 rows. Both used one outstanding grant,
532672 encoded / 533204 decoded bytes. Two default hosts filled the 512 MiB
reservation and a third was refused with `MemoryLimit`. An exclusive 8 MiB row
used one grant, 8389056 encoded / 8389517 decoded bytes; sampled aggregate RSS
was 19955712 baseline, 70778880 peak, 50823168 delta. The larger overlap is
reported separately from the normal-batch target. Parent-store retained-overhead
ratio, native DuckDB budgets and reference-machine repeats remain M2 checkpoint
work; this change does not claim measurements of those deferred metrics.

Default-feature isolation also passed
`cargo check --locked -p openrows-driver-host --no-default-features` (exit 0,
3.53 s). Normal dependency graphs contain no native SQLite in the default host
and no host client/helper or DuckDB in the desktop. New direct dependencies reuse
the existing Tokio version for result-store notifications and internal SQL/facet
and test crates; no third-party package/version was added to the lockfile.
Temporary verification logs and generated desktop dist were removed. Owned
helpers are reaped, PostgreSQL TLS directories are gone, and created containers
are removed. The local Rust build cache and `orows-linux-target` remain.

## Validation / evidence — 2026-10-10, M2-01c

Installed packaging now uses Tauri `bundle.externalBin`, staged from an actual
`--no-default-features` helper build. No placeholder is shipped. The default
helper has no database engine: the typed Probe target authenticates, advertises
empty capabilities and refuses Connect with Unsupported. M2-04 adds the host-only
DuckDB registration in the typed driver id, exact Hello contract and registry.
Desktop app state owns lazy installed resolution, the private cache/hosts root,
256 MiB per-host / 512 MiB aggregate admission and retained per-profile/driver
three-crashes-in-60-seconds circuits. Resolution never searches PATH/cwd/env;
the absolute-path debug override and its environment read are compiled out of
release builds.

Missing, directory and non-executable helpers return Spawn; scripts, foreign
architectures, unrelated native executables and version mismatch return
Incompatible. Both retain their specific user messages through DriverError and
DriverHost IPC errors. Version-mismatch tests authenticate both lanes, reap the
helper, remove endpoints and refuse the fourth launch without spawning. The
typed single driver argument is covered by the secret-free command-surface test.

`nice -n 10 just build-app` produced `Contents/MacOS/openrows-driver-host` in
OpenRows.app, with the production helper staged automatically. Local macOS uses
Tauri's ad-hoc identity `-`; source inspection of the pinned Tauri CLI 2.12.1
confirms that the chosen identity signs external binaries before the main app
and outer bundle. `codesign -dv --verbose=4` recorded the following for both:

```text
Format=app bundle with Mach-O thin (arm64)  # app; helper: Mach-O thin (arm64)
CodeDirectory ... flags=0x10002(adhoc,runtime)
Signature=adhoc
TeamIdentifier=not set
Runtime Version=26.5.0
```

`codesign --verify --deep --strict OpenRows.app` exited 0. No extra entitlements
were added. Developer ID is supplied through APPLE_SIGNING_IDENTITY; the stable
dev runner signs both adjacent helper and app when the owner's identity exists.
Neither certificate-backed signing nor notarization/stapling was run locally.

The packaged smoke used the same GUI-free resolver/runtime as desktop state,
with the existing absolute CARGO_TARGET_DIR:

```sh
OPENROWS_PACKAGED_SMOKE=1 \
OPENROWS_PACKAGED_APP="$CARGO_TARGET_DIR/release/bundle/macos/OpenRows.app" \
nice -n 10 bash scripts/sccache-guard.sh cargo test --release --locked \
  -p openrows-host-client --test packaged -- --nocapture
```

Hardware: Apple M4 (Mac16,10), 24 GiB RAM, macOS 26.5 (25F71), arm64,
Rust 1.94.0. Each run measures resolution + spawn + both-lane handshake with
parent Instant, then waits for Closed and verifies endpoint removal. Warm
statistics are nearest-rank percentiles of 29 subsequent launches; no database
connect or native engine startup is included. These are local samples, with no
main baseline, cache eviction, fresh-machine or reference mac-low certification.

| Packaged fixture | First ms | Warm p50 ms | Warm p95 ms | Warm max ms | Deadline ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| First helper launch from newly built app | 471.838 | 3.402 | 4.306 | 7.860 | 2000 |
| Fresh `ditto` app copy; helper quarantine `0081;67000000;test-only;` retained | 378.278 | 3.359 | 5.870 | 10.223 | 2000 |
| Final source rebuilt app verification | 467.148 | 3.640 | 6.426 | 9.182 | 2000 |

All three runs printed `test result: ok. 1 passed; 0 failed; 0 ignored`; each
run's 30 samples closed cleanly and removed their endpoints. The second command used
the copied app path and confirmed quarantine remained after launch. Codesign
inspection and compilation had already warmed some OS/file caches; this does
not certify Gatekeeper behavior for a downloaded Developer ID release. Both
initial first launches exceed the ADR's 150 ms cold-start target, while every sample
meets the production 2 s deadline. The target miss remains a measured checkpoint
gap; no deadline increase or retry was added.

Production logical file sizes: app 69,677,875 bytes (66.450 MiB), helper
3,339,296 bytes. `CI=true nice -n 10 pnpm --filter @openrows/desktop tauri bundle
--bundles dmg` produced OpenRows_0.1.0_aarch64.dmg at 28,157,425 bytes
(26.853 MiB), below the docs/13 installer budget of 40,000,000 bytes. The
uncompressed app exceeds 40 MB; that budget applies to the distributed installer.
The final rebuild again passed `just build-app` with the staged directory
removed beforehand (4m 24s app build). Its helper was byte-identical to the
measured production helper. Direct `tauri bundle` uses the existing staged binary:
stage release before
creating release installers, including after debug-profile checks.

Linux copied source into `/work` in `rust:1.94.0-bookworm` (aarch64), with
`--init --cpus 4`, four Cargo jobs and four test threads, rather than a macOS
socket-incompatible bind mount. The protocol/client/helper crates passed 81
tests, zero failures/ignored; the packaged opt-in test returns without launching
an artifact when disabled. Installed resolver tests passed eight; packaging
tests passed two; host lifecycle tests passed 34 and conformance passed ten.
Selected-crate Clippy with all targets and `-D warnings` exited 0 (20.33 s).
The container's unavailable sccache used the existing guard's wrapper bypass.
The final test repeat reused the retained shared Linux cache through a local
target-directory link. No GUI/WebKit was required. An initial new probe-test
assertion failed because
it omitted the Accepted control response; the assertion was corrected before
the complete Linux rerun. Linux containment retained the accepted monitoring-only
fallback because cgroup v2 was not delegated.

After deleting the staged binary directory separately before each recipe,
`just gen-ipc`, `just lint`, `just test` and `just ipc-check` staged the helper
automatically and exited 0. Ubuntu CI now runs `bash scripts/stage-host.sh dev`
before workspace Clippy/tests. Binding generation made no tracked contract
change. Real final result lines were:

```text
just lint:      Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.33s
just test:      Summary [  26.330s] 1319 tests run: 1319 passed, 196 skipped
just ipc-check: Summary [   0.101s] 1 test run: 1 passed, 249 skipped
UI tests:      230 files / 2701 tests passed (107.12 s)
bench-ui:      initial JS 301297 gzip bytes / 350000; CSS 6306 / 15000
bench-test:    Ran 89 tests in 0.641s; OK
script-test:   Ran 6 tests in 0.791s; OK
```

Rust formatting, workspace Clippy (`-D warnings`), eslint, frontend formatting,
TypeScript, knip, unused dependencies, advisories/licences/bans/sources, hygiene
and UI tokens all passed. Workspace Clippy initially caught missing doc-markdown
backticks in the new desktop module; the comment was fixed before the green run.
Normal-edge dependency graphs confirm no native SQLite in the production helper
and no helper/DuckDB/ADBC/ODPI native dependency in desktop. Desktop adds only
the existing internal Apache-2.0 host-client crate; no third-party dependency
or version was added.

`cargo check --locked --target x86_64-pc-windows-msvc -p openrows-host-protocol
-p openrows-host-client -p openrows-driver-host --no-default-features` exited
101 at ring 0.17.14: `fatal error: 'assert.h' file not found`. Windows PE/`.exe`
selection and debug-only override gates were reviewed by inspection; Windows
compilation/runtime acceptance remains unverified.

Verification logs, the fresh quarantined copy and generated frontend dist were
removed. Created Linux containers were stopped/removed; smoke helpers reached
Closed and removed their endpoints. Existing shared caches, the ignored staged
helper and final app/DMG build artifacts remain available for review.

Named release/checkpoint gaps: owner's Developer ID signature and same-team
verification, notarization/stapling and downloaded-app Gatekeeper validation;
stable dev-identity signing on a machine with that certificate; Windows MSVC
compile verification (local ring/assert.h toolchain limitation) and Windows/Linux
installed package launch; reference-machine/certificate-backed cold startup
against 150 ms; universal assembly; M2-04 DuckDB registration and engine-specific
measurements. M2-01 stays partial in docs/27. No native engine was moved into the
desktop, no user-visible hosted driver was registered and no key was invented.

## Validation / evidence — 2026-10-10, M2-01c review fixes

The lead's staging → workspace gate exposed a production/test binary collision.
Native staging built the no-SQLite helper into the same top-level Cargo profile
path used by host tests, and cached workspace builds did not reliably re-uplift
the feature-enabled image. Staging now always passes `--target <triple>` and
builds under `$CARGO_TARGET_DIR/<triple>/<profile>/` (`debug` for `dev`), within
the same cache and with sccache retained. The first reproduction after that
change exposed a second overwrite: Tauri copies external binaries beside the
app in the top-level profile directory, replacing `openrows-driver-host` again.

Tests and benchmarks now use `CARGO_BIN_EXE_host-test-fixture`, a separately
named binary gated by `test-sqlite`. It shares the production process entry,
including panic diagnostics, core-dump suppression, runtime and exit behavior;
only its test feature graph differs. Production staging still builds exactly
`--bin openrows-driver-host --no-default-features`. Tauri's copied helper,
dev signing and installed resolution retain the production name and layout.
Benchmarks obtain the fixture directly from Cargo instead of `OPENROWS_HOST_BIN`.

Five stub-compiler staging tests exercise native dev/release/custom profiles,
an environment-selected Linux target and an explicit Windows target overriding
the environment. They verify distinct output paths/files, preservation of the
test registry and wrapper/jobs, and the production feature flags. Running them
against the HEAD script fails all three native-path checks as expected; the
fixed script passes all five. `just script-test` runs these alongside the six
existing wrapper checks, and Ubuntu CI runs the new guard after staging and
before workspace Clippy/tests. The normal Rust packaging suite additionally
checks distinct canonical paths and, on Unix, device/inode identity between the
fixture and production sidecar; conformance proves the SQLite registry works.

The exact macOS reproduction, with four build jobs/test threads and nice 10,
now passes:

```sh
bash scripts/stage-host.sh dev
cargo nextest run --locked --workspace -E 'binary(conformance) & package(openrows-driver-host)'
```

```text
Summary [   0.680s] 10 tests run: 10 passed, 0 skipped
```

The production two-second deadline and existing release/checkpoint gaps above
remain unchanged. No dependency or native driver was added by these review fixes.

`nice -n 10 just build-app` rebuilt the app (4m 37s desktop build), containing only the production helper: 3339472 bytes; app logical size 69678051 bytes. Both carry `flags=0x10002(adhoc,runtime)`; deep/strict verification passed. The opt-in release packaged command above passed one test / 30 clean cycles: first 559.332 ms, warm p50/p95/max 3.514/5.494/7.202 ms against 2000 ms, on the same M4/24 GiB/macOS 26.5 host. No cache eviction or main comparison was performed; the 150 ms first-launch target is still missed. No installer was rebuilt this round.

`nice -n 10 just dev` staged from the triple-specific debug path and launched the existing runner. Both adjacent app/helper now verified with `Authority=OpenRows Dev` and strict signature checks, closing the local stable-dev-identity gap. The unchanged frontend dev monitor then raised `TypeError: Attempted to assign to readonly property` at `devMonitor.ts:82` (`internals.invoke`); this separate dev-UI defect was left untouched. The app/Vite/runner processes were stopped after the staging/signing check.

Final gates ran once in the requested order: `just gen-ipc`, then fmt-check → lint → test → deny → machete → hygiene → ui-check → bench-ui, frontend checks, then ipc-check. The staged native helper was deleted before lint to verify automatic restaging. Workspace Clippy finished in 7.69s; workspace nextest exited 0 with 1320 tests / 196 configured skips. All ten host conformance tests and the three packaging checks passed in that workspace run. Advisories/bans/licences/sources and unused-dependency checks passed; tokens were fresh and component colours used tokens. Initial JS was 301306 gzip bytes / 350000; component CSS 6294 / 15000.

Frontend eslint, Prettier, TypeScript and knip exited 0; Vitest passed 230 files / 2701 tests in 93.47s. `just ipc-check` printed `Summary [   0.093s] 1 test run: 1 passed, 249 skipped`. IPC generation changed no contract. Two earlier unchanged concurrent packaging mismatch runs timed out; the isolated repeat and complete repeated packaging suite passed, followed by the green final workspace gate. No deadline or mismatch-test behavior was changed. Remote Ubuntu CI was not triggered; its staging/guard/Clippy/workspace-nextest order was checked locally.

Generated frontend dist and staging-test scratch directories were removed; no verification log or screenshot was retained. Dev processes were stopped, and no owned helper or new container remains. The configured target cache, ignored staged binary and app artifacts are retained for review; HEAD stays 7753ace with uncommitted fixes.
