# 0007 — One actor per database session

- Status: Accepted — validation pending (M1-01, M1-11)
- Date: 2026-10-05; S3 evidence 2026-10-07; reviewed 2026-10-07 (M0-11)

## Context

Database connections process one statement at a time, many driver connection types are not `Sync`, and we need reliable cancellation, panic containment, and clear state (idle, busy, in transaction, broken).

## Decision

Each live connection is owned by a single tokio task (**session actor**) that receives commands over an `mpsc` channel and replies via `oneshot` channels. Cancellation handles are obtained before execution and held outside the actor. Blocking drivers run their actor loop on a dedicated OS thread.

See [04-core-engine.md §3](../04-core-engine.md#3-session-actors).

## Consequences

- ✅ No locks around connections; state machine is explicit and testable.
- ✅ A driver panic kills only its actor; the manager observes it and recovers.
- ✅ Natural queueing semantics visible to the UI.
- ⚠️ Slight overhead of message passing (negligible compared with network round trips).
- ⚠️ Must avoid deadlocks from actors awaiting each other — rule: actors never call other actors synchronously.

## Alternatives considered

- **Shared connection behind `Arc<Mutex<_>>`:** simple, but cancel would contend on the lock, and state transitions spread across callers.
- **Connection pool only (e.g. deadpool/bb8):** good for servers, but a GUI needs pinned sessions per tab with transaction affinity and visible state.

## M0-S3 evidence — 2026-10-07

The draft `openrows-driver-api::Session` fits this model. It is `Send` but not `Sync`,
`execute` takes `&mut self` so only the owning actor can run a statement, and
`close(self: Box<Self>)` consumes the connection. Boxed `Send` futures keep
`Box<dyn Session>` object-safe without an `async-trait` dependency
(`boxed_session_is_object_safe_and_send`). The prototype PostgreSQL session holds
one TCP stream and one read buffer, with no locks. Server errors, decode errors and
a sink `Stop` all drain the connection to `ReadyForQuery` before returning. A
following statement on the same session succeeds against PostgreSQL 12, 17 and 18
(`errors_are_typed_and_the_session_stays_usable`,
`statements_without_rows_and_stopped_streams`). Out-of-band cancel and the actor
loop are not exercised yet (M1-01, M1-11). This evidence supports, and does not
change, the accepted decision.

## Validation / evidence — 2026-10-07 (M0-11 review)

Accepted; no revision. The M0-S3 evidence above (#25) validates the session contract the actor will own. M0-S3 drove the wire protocol with `postgres-protocol` instead of `tokio-postgres` ([05 §11](../05-database-drivers.md#11-m0-s3-postgresql-streaming-spike)); this ADR names no driver library, so that finding does not affect it. The remaining consequences are pending:

- **M1-01** (session manager): actor loop, command queue, panic isolation and cancellation tokens. Its gates test that a panicking driver task does not take down the app and that a cancel request is sent ≤ 200 ms after the command.
- **M1-11** (PostgreSQL driver): out-of-band cancel through the PostgreSQL cancel request, obtained before execution and held outside the actor.
- Blocking drivers on a dedicated OS thread: no M1 task names this yet; the first candidate is the SQLite driver (M1-13).
