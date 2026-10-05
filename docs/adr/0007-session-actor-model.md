# 0007 — One actor per database session

- Status: Accepted
- Date: 2026-10-05

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
