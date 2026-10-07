# Architecture Decision Records

| # | Title | Status | Evidence / pending validation |
| --- | --- | --- | --- |
| [0001](0001-rust-core-with-tauri-2.md) | Rust core engine with Tauri 2 shell and React UI | Accepted — validation pending (M0-S6b, M0-S5, X-02) | On the M4: S1 tile p95 1.00 ms (#20), S6 warm start p50 ≈ 290–330 ms and idle ≈ 122–134 MB (#16, #18); mac-low runs pending: startup/memory M0-S6b, editor M0-S5, IPC X-02 |
| [0002](0002-arrow-ipc-for-results.md) | Apache Arrow for result storage and IPC transport | Accepted | M0-S1 (#20): 256×20 tile p95 1.00 ms (≤ 3 ms) on the M4; M0-S3 (#25) drivers decode into Arrow; mac-low re-run is a revisit trigger (X-02) |
| [0003](0003-codemirror-6-editor.md) | CodeMirror 6 for all code editors | Accepted — validation pending (M0-S5) | Implementer M4 runs p95 sql 9–11 ms, vim 9–10 ms (≤ 16 ms) (#27); maintainer `just bench-editor` run, manual IME check and mac-low run pending (M0-S5, `hw`) |
| [0004](0004-canvas-data-grid.md) | Canvas-rendered data grid behind an adapter | Accepted | M0-S2 (#23): Option A, best repeat 0 frames > 32 ms on the M4; production grid in M1-19 |
| [0005](0005-apache-2-license.md) | Apache-2.0 licence and DCO | Accepted | In effect since the first commit: `LICENSE`, `NOTICE`, SPDX headers, `cargo-deny` allowlist in `just check`, sign-off on every commit; CI DCO and npm licence checks with X-01 |
| [0006](0006-file-based-api-collections.md) | File-per-request YAML collections for the API client | Accepted — validation deferred (M4-02) | No spike; design decision validated by M4-02 and journey 7 (M4-09) |
| [0007](0007-session-actor-model.md) | One actor per database session | Accepted — validation pending (M1-01, M1-11) | M0-S3 (#25): `Session` contract (`Send`, not `Sync`, `&mut self`) and session reuse after errors on PG 12/17/18; actor loop, panic isolation and cancel pending |
| [0008](0008-quickjs-script-sandbox.md) | QuickJS sandbox for API scripts | Accepted — validation deferred (M4-05) | No spike; design decision validated by M4-05 (sandbox limits, restricted mode) |
| [0009](0009-native-ai-byok-cli-bridges-mcp.md) | Native AI with BYOK, agent-CLI bridges and MCP | Accepted — validation deferred (M45-09) | No spike; design decision validated by M4.5 tasks M45-01..M45-08 and the M45-09 exit gate |

Last reviewed: 2026-10-07 (M0-11). Task IDs refer to [27 — Delivery plan](../27-delivery-plan.md).

## Status values

The status line starts with one of the values below. An accepted ADR whose validation is not complete carries one of two qualifiers that name the task(s) producing the missing evidence.

| Status | Meaning |
| --- | --- |
| Proposed | Under discussion; nothing may depend on it yet. |
| Accepted | Decision in force; its validation criterion is met and the evidence is recorded in the ADR. Revisit triggers (for example a later run on other reference hardware) may still be listed. |
| Accepted — validation pending (IDs) | Decision in force and work may build on it. Spike evidence exists but part of the validation criterion is still open; the named tasks close it. |
| Accepted — validation deferred (ID) | Decision in force for design and planning. No spike was needed or possible before implementation; the named task validates it. |
| Superseded by NNNN | Replaced by a newer ADR. |
| Deprecated | No longer applies and nothing replaces it. |

If pending or deferred validation misses its criterion, the ADR is revised explicitly (new dated evidence section and status) or superseded. Decisions are never edited silently. When a doc and an ADR disagree, the newer accepted ADR wins and the doc gets fixed ([docs/README.md](../README.md#conventions)).

## Template

```markdown
# NNNN — Title

- Status: Proposed | Accepted | Accepted — validation pending (IDs) | Accepted — validation deferred (ID) | Superseded by NNNN | Deprecated
- Date: YYYY-MM-DD
- Deciders: names/handles

## Context
What forces are at play? What problem are we solving?

## Decision
What we decided, stated plainly.

## Consequences
Positive, negative, and follow-up work.

## Alternatives considered
Each with the reason it was not chosen.

## Validation / exit criteria
How we will know this was right, or when to revisit.

## Validation / evidence — YYYY-MM-DD
Evidence (PRs, benchmark names, numbers) or the task IDs that will produce it.
```
