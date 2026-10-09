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
| FFI driver segfault | Driver host process dies | Proxy sees EOF → sessions on that host Failed; host respawned on next connect |
| Network drop (idle) | Keepalive detects | Auto-reconnect with backoff (1 s, 2 s, 4 s … max 30 s, 5 attempts); startup commands replayed |
| Network drop (in query) | Query fails with `Network` error | Not retried; user can rerun |
| Network drop (in transaction) | Session → `Broken` | Clear message: "transaction was rolled back by the server"; staged change set preserved |
| Script infinite loop | QuickJS interrupt handler at time limit | Request marked failed with "script timeout" |
| Webview crash / reload | Core unaffected | Webview reloads; UI state restored from core (tabs, drafts, results still in store) |
| Core panic outside a session | Global panic hook logs (scrubbed), the crash marker of the run stays behind | Next launch shows "OpenRows closed unexpectedly" listing journaled staged edits, with "Open logs folder" (M1-28); the edits come back through the table's own restore offer (D-09); opt-in crash report (stub only) |

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
