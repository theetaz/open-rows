# 14 — Reliability and Observability

## 1. Principles

1. **Never lose user work.** Editor text, pending change sets, open tabs and API request drafts are persisted continuously.
2. **Fail small.** A failure in one session, driver, request or panel must not take down the window or the app.
3. **Never retry a write behind the user's back.**
4. **Every wait is bounded and cancellable.**
5. **Explain failures in user terms** with a precise technical detail one click away.

## 2. Failure domains

```mermaid
flowchart TB
    App["App process"] --> W1["Window"]
    W1 --> T1["Tab"]
    T1 --> S1["Session actor"]
    S1 --> D1["Driver (in-process)"]
    S1 -.-> H["Driver host process (FFI drivers)"]
    App --> HTTP["API request tasks"]
    App --> SCR["Script sandbox contexts"]

    classDef iso fill:#eef,stroke:#88a
    class S1,H,HTTP,SCR iso
```

| Failure | Containment | Recovery |
| --- | --- | --- |
| Driver returns error | Query fails, session stays usable | Error shown inline; session continues |
| Driver panics | Session actor task ends (`JoinError`) | Session marked Failed; "Reconnect" offered; panic logged with correlation id |
| Hosted driver exit, native crash, memory limit or broken protocol | Only its one session host is terminated/reaped; endpoints removed | Typed `DriverHost`; explicit new connect only, with no statement/startup/COMMIT replay (M2-01a supervisor; M2-01b actor/metadata integration) |
| Network drop (idle) | Keepalive detects | Auto-reconnect with backoff (1 s, 2 s, 4 s … max 30 s, 5 attempts); startup commands replayed |
| Network drop (in query) | Query fails with `Network` error | Not retried; user can rerun |
| Network drop (in transaction) | Session → `Broken` | Clear message: "transaction was rolled back by the server"; staged change set preserved |
| Script infinite loop | QuickJS interrupt handler at time limit | Request marked failed with "script timeout" |
| Webview crash / reload | Core unaffected | Webview reloads; UI state restored from core (tabs, drafts, results still in store) |
| Core panic outside a session | Global panic hook logs (scrubbed), the crash marker of the run stays behind | Next launch shows "OpenRows closed unexpectedly" listing journaled staged edits, with "Open logs folder" (M1-28); the edits come back through the table's own restore offer (D-09); opt-in crash report (stub only) |


**Implemented (M2-01a):** the Tauri-free supervisor publishes `HostState::Failed` after targeted kill/reap and rendezvous cleanup, including when idle. A 256 MiB RSS threshold is sampled every 100 ms independently of data admission; host reservations total at most 512 MiB in one parent. A missing or unreadable RSS sample never becomes `MemoryLimit`; only an observed sample above the threshold triggers a monitored memory-limit failure. Child exit and heartbeat detection remain independent. macOS containment is monitoring-only with force-kill, and can overshoot between samples. Heartbeats arrive every second; three missed intervals plus a 1 s probe identify an unresponsive control runtime. A detected suspend/clock gap resets the health deadline. A responsive control runtime does not impose a query deadline. Explicit Cancel/Stop and Close retain the 2 s drain/shutdown grace. Abrupt EOF, including a partially written transport frame, is `Exited`; malformed payloads are `Protocol`. Three abnormal exits per shared profile/driver circuit in 60 s refuse a fourth spawn; there is no respawn loop.

The `DriverHost` error kind is not `Network` and `DriverError::is_connection_lost()` is false. Missing write/COMMIT acknowledgement remains uncertain; a host failure supplies no rollback guarantee. M2-01b wires the lifecycle observer into idle actors and metadata slots; busy loss ends accepted partial results and fails queued callers exactly once. IPC and session events show “The driver process stopped; reconnect to continue”. Hosted sessions never idle-suspend or invoke normal network restore/replay; a fresh explicit connect is required. Installed sidecar identity and Windows/Linux release smoke remain M2-01.4/platform work.

### 2.1 Panic policy

- `panic = "unwind"` in release so session-level panics are catchable at task boundaries.
- A global panic hook records the panic message (clipped and scrubbed), location, thread and a clipped backtrace to the log, then runs the previous hook. Correlation ids are not implemented yet.
- Crash marker (`run-marker.json` in the app-data dir): written at startup with the process id and removed at a clean exit. A marker whose process is gone at the next launch means the last run did not exit cleanly. A marker owned by a live instance is left alone.
- Code in the core must not `unwrap()`/`expect()` on data from external sources; lint (`clippy::unwrap_used` in non-test code, with explicit allowances) enforces this.

## 3. Persistence of in-progress work

| Item | When persisted | Where |
| --- | --- | --- |
| Editor text (scratch tabs) | 300 ms debounce after change, on blur, on quit | Local store `drafts` table |
| File-backed tab unsaved changes | Same | Local store `drafts` (does not overwrite the file until ⌘S) |
| Pending change sets | Every mutation | Local store `change_sets` |
| Tab layout and view state | 1 s debounce | Local store `workspace_state` (per connection/database; Project file drafts remain file-backed) |
| API request edits (unsaved) | 300 ms debounce | Local store `drafts` |

All writes go through a single writer task with WAL mode; batched to avoid fsync storms.

## 4. Timeouts (defaults, all configurable)

| Operation | Default |
| --- | --- |
| DNS | 5 s |
| TCP connect | 10 s |
| SSH handshake + auth | 20 s (excluding user prompts) |
| TLS handshake | 10 s |
| Database auth | 15 s |
| Introspection query | 30 s |
| User query | none (cancel button), optional per profile |
| Keepalive interval | 30 s |
| API request | 0 = none by default, configurable per collection |
| Script run | 5 s |
| Graceful shutdown | 2 s |

## 5. Connection diagnostics

"Test connection" runs a staged probe and reports each step:

```text
✓ Resolve db.internal.example.com → 10.0.4.12            12 ms
✓ SSH bastion.example.com:22 (ed25519 key, agent)          184 ms
✓ Tunnel to 10.0.4.12:5432                                  21 ms
✓ TLS 1.3 (TLS_AES_128_GCM_SHA256), cert valid for db.internal.example.com
✗ Authentication failed: password authentication failed for user "app" (28P01)
  Hint: check the password, or whether pg_hba.conf allows this user from the bastion's IP.
```

Hints come from a curated table keyed by error kind + database code.

## 6. Logging

Implemented in `apps/desktop/src-tauri/src/observability/` (M1-28):

- `tracing` with structured fields, written as JSON lines to the app log folder (`~/Library/Logs/com.prabhavalabs.openrows/` on macOS, Tauri's `app_log_dir`). One file per day (`openrows.YYYY-MM-DD.log`), seven kept. A 50 MB size cap is not enforced yet; `tracing-appender` rotates by day only.
- Levels: `error`, `warn`, `info` (default), `debug`, `trace`. `OPENROWS_LOG` overrides `diagnostics.logLevel` in `settings.json`; both are read at launch, so a change applies at the next launch (no runtime switch yet). Unknown values fall back to `info`.
- **Field discipline first.** Call sites log typed kinds (`IpcErrorKind`, `ErrorKind`, `ScriptOutcome`), counts, durations and identifiers. They never log SQL text, its literals (not even at `trace`, which is stricter than the first draft of this section), row values, connection URLs, hosts, user names, or driver and engine error messages (Postgres messages can echo row values).
- **Scrubbing layer** (`observability/scrub.rs`) before the file writer, on every line: URL userinfo passwords, sensitive query parameters (`password`, `token`, `key`, `secret`, `sig`, …), JSON and `key=value` fields whose names look secret (also inside escaped JSON), and bearer tokens. Scrubbing is a second line of defence, not the primary control.
- Spans: `session.open` and `session.close` and `query.run` and `export` in `openrows-core`; `table.commit` and `connection.test` (with a debug event per test step) in the app. Outcomes are logged as kinds and counts only.
- Tests prove the guarantee: a password in a URL, a field or free text never reaches the file (`observability/tests.rs`), and a SQL literal run through the query runner never reaches the log (`openrows-core` `tests/observability.rs`).
- Webview errors, unhandled rejections and timings reach the log through the typed `ui_report` command under the `ui` target. The webview rate-limits them to 30 per minute.
- Dev builds only: main-thread tasks of 50 ms or more (`longtask`, or frame gaps where WebKit provides no `longtask` entries) and IPC commands of 100 ms or more are reported the same way.
- Not built yet: the in-app log viewer, the "Export diagnostics bundle" action, and a runtime log-level switch.

## 7. Crash and error reporting

- **Off by default.** First-run screen asks; can change in Settings.
- Current state (M1-28): Settings → Diagnostics has a "Send crash reports" switch that defaults to off and only stores the choice. OpenRows sends no telemetry and no crash reports, and no upload code exists. The first-run screen and the opt-in reporter are not built.
- When enabled: native crash (minidump via `crashpad`/`sentry-rust`-compatible self-hostable backend) and Rust panic reports. Endpoint is configurable; the project's default endpoint is a self-hosted, open-source crash collector.
- Payload excludes query text, values, hostnames, usernames, file paths in home dir (rewritten to `~`).
- User can view exactly what will be sent.

## 8. Health and metrics (local only)

- Internal metrics (counters/histograms via `metrics` crate) kept in memory: query latency, decode throughput, result store usage, spill bytes, IPC payload sizes, frame times reported by the UI.
- Visible in a Diagnostics panel (useful for users filing perf issues). Never sent anywhere.

## 9. Upgrades and data migration safety

- Local store schema migrations are versioned, forward-only, run in a transaction, and preceded by a backup copy of the DB file (keep last 3).
- Settings and profile files carry a version; unknown fields preserved.
- If a migration fails, the app starts in a safe mode with the backup and shows a recovery dialog instead of crashing.

## 10. Implementation notes (M1-08 reconnect and resilience)

| Area | What is implemented |
| --- | --- |
| Session states | `SessionState::Reconnecting` between `Idle`/`Busy` and `Failed`; the `session-state-changed` event carries the cause. The UI marks the database `reconnecting` (amber rail dot, "Reconnecting…" label, tab bar note that queries wait) and, when the session fails after being open, `lost` (red dot, "Connection lost", tab bar Reconnect button that reopens the database). |
| Keepalive | `SessionManager::open_resilient` keeps the connect recipe (`Resilience`: reconnect closure, dialect, `RetryPolicy`). An idle session pings (`Session::ping`: one simple-query round trip on PostgreSQL, `SELECT 1` elsewhere) every keepalive interval (default 30 s; `general.keepaliveSeconds` in settings.json, 15 s – 2 min or off in Settings ▸ General) and before a statement after 10 s idle. A ping that fails with a transport error or does not answer within 10 s means the connection is lost; a server error (e.g. an aborted transaction) still proves it alive. Every TCP socket (direct, proxy, first SSH hop) has OS keepalive (30 s idle, 10 s probes) so a busy session's dead peer is noticed too; SSH sessions keep russh's 30 s keepalive. Local files (SQLite) are not probed. |
| Wake and network change | `openrows_net::watch_path` samples every 2 s: wall time advancing ≥ 4 s beyond the monotonic clock (which stops during sleep) is a wake; a changed local address of the default IPv4/IPv6 route (UDP `connect` to a documentation address, no packet sent) is a network change. Either one pings every shared SSH tunnel (`SharedTunnel::probe`, 5 s; a dead one is dropped so the next dial reconnects), drops idle metadata connections and makes every session probe at once (`SessionManager::probe_all`). |
| Reconnect | A lost idle session reconnects through the same route and target (proxy, shared tunnel, TLS, database) with backoff 1 s, 2 s, 4 s … max 30 s, 5 attempts; network errors and servers that are starting, shutting down or full (SQLSTATE classes 08, 53, 57; MySQL 1040) are retried, authentication and TLS failures stop at once. The latest `SET [SESSION] search_path`/`SET SCHEMA` (PostgreSQL) or `USE` (MySQL/MariaDB) run outside a transaction is replayed; if replaying fails the session fails rather than run statements in the wrong place. Statements queued meanwhile run on the new connection; when every attempt fails they fail with the reason. Closing the session stops the attempts. Metadata connections idle for 10 s are pinged before a catalog read and replaced when dead. |
| Never retry | A statement whose connection dies while it runs fails with "Connection lost while the statement was running: it may or may not have been applied, and it was not retried" (plus "the open transaction was rolled back by the server, unless this statement was its COMMIT" inside a transaction); a lost connection also ends the rest of a script, even without Return on Error. A connection lost while the server held a transaction (PostgreSQL `ReadyForQuery` status, MySQL `SERVER_STATUS_IN_TRANS`) is not replaced: the session fails with "the server rolled the transaction back. Reconnect to continue." A table-edit or structure commit whose connection drops before `COMMIT` reports "the connection was lost before COMMIT, so the server rolled the transaction back and nothing was changed"; a lost `COMMIT` acknowledgement stays the uncertain `08007`. Either way the staged change set is kept. |
| Tests | Paused-time unit tests on a scripted server (`openrows-core` `tests::resilience`: detection within the interval, half-open probe timeout, in-flight write applied once, script stops, lost transaction fails the session, `COMMIT` in flight, search-path replay, give-up after N attempts with queued statements failed, wake probe, probe before a stale statement, close while reconnecting). `just test-pg`: `pg_terminate_backend` of an idle and of a writing backend, a black-holed and a refusing-then-recovering link through `openrows_net::testing::FaultyLink`, and an edit commit cut mid-transaction. `just test-mysql` (MySQL 8.4, MariaDB 11.4): `KILL` of an idle (with `USE` replay) and of a writing connection, and an edit commit killed mid-transaction. `just test-ssh`: a paused SSH server fails the tunnel probe within its timeout and a cut tunnel at once; both reconnect on the next dial. |
| Not covered | Restarting a real database container in the tests (the throwaway containers publish ephemeral ports that change on restart and are shared by parallel tests; a refusing `FaultyLink` stands in). OS power/network notifications (`NSWorkspace`, `NWPathMonitor`) are not used; the clock-gap and route heuristics need no platform code. A `SET` inside a transaction that later commits is not replayed. |

### Implemented (M2-01c): installed startup failures

A missing/non-file/non-executable helper fails with Spawn and “driver host
could not start; reconnect explicitly”. A wrong native format/architecture,
early child exit or version/build contract mismatch fails with Incompatible
and “driver host is incompatible with this app”. The IPC category is DriverHost;
startup messages retain the specific reason. Cleanup kills/reaps a started child
and removes its rendezvous directory before returning handshake failure.
There is no PATH/cwd/env search, download, automatic retry or in-process fallback.
The shared runtime retains crash circuits by profile/typed driver. Installed
macOS smoke is opt-in; Developer ID/notarization and Windows/Linux package
launch remain named release gates in ADR-0010 and docs/27.
