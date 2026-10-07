# 0001 — Rust core engine with Tauri 2 shell and React UI

- Status: Accepted — validation pending (M0-S6b, M0-S5, X-02)
- Date: 2026-10-05; reviewed 2026-10-07 (M0-11)

## Context

We need a cross-platform (macOS first, then Windows and Linux) database and API client that is as fast and light as native paid tools, while being buildable by an open-source community. UI must be rich (code editor, data grid, diagrams) and themable.

## Decision

- All protocol, data and storage logic in a set of **Rust crates** with no UI dependency (`openrows-core` and friends).
- Desktop shell built with **Tauri 2**, using the system webview.
- UI in **React + TypeScript**, built with **Vite**.
- The CLI (`openrows`) reuses the same crates.

## Consequences

- ✅ Small binaries (system webview), low idle memory compared with bundled Chromium.
- ✅ Memory-safe, fast core; excellent async ecosystem (tokio) and pure-Rust drivers for most databases.
- ✅ One UI codebase for three OSes; large contributor pool for React/TS.
- ✅ Core testable headlessly and reusable in CLI/CI.
- ⚠️ Three different webview engines to test (WebKit, Chromium, WebKitGTK); WebKitGTK is the weakest.
- ⚠️ Not truly native widgets; native feel must be earned through careful design (menus, shortcuts, vibrancy, focus behaviour).
- ⚠️ IPC boundary must be designed carefully for large data (see ADR-0002).

## Alternatives considered

- **Electron + Node core:** fastest to build, but ~150–300 MB memory baseline and large installers conflict with efficiency goals; Node drivers vary in quality for streaming large results.
- **Rust core + native UI per OS (SwiftUI/AppKit, WinUI, GTK):** best native feel, but three UI codebases triples cost and slows the community; feasible later for specific surfaces if needed.
- **Flutter / Qt:** Qt licensing and C++ contributor pool; Flutter's text editing and code-editor ecosystem is weaker for our needs.

## Validation

M0 spikes S1 (IPC), S5 (editor) and S6 (startup) must meet budgets on mac-low. If startup or IPC budgets cannot be met, revisit.

## Validation / evidence — 2026-10-07 (M0-11 review)

The decision stands. All three spikes meet their budgets on a Mac mini M4 (release build), which is not the mac-low reference machine ([13 §1](../13-performance.md#1-reference-hardware)), so the criterion above is only partly met.

| Spike | Budget | M4 evidence | mac-low |
| --- | --- | --- | --- |
| S1 IPC | 256×20 tile p95 ≤ 3 ms | `just bench-ipc`: total p95 1.00 ms in both maintainer runs (#20, [ADR-0002](0002-arrow-ipc-for-results.md#m0-s1-evidence--2026-10-06)) | pending; no dedicated task, X-02 records mac-low baselines |
| S5 editor | keystroke p95 ≤ 16 ms | `just bench-editor`: p95 sql 9–11 ms, vim 9–10 ms in four implementer runs (#27, [ADR-0003](0003-codemirror-6-editor.md#m0-s5-evidence--2026-10-07)) | pending, part of M0-S5 (`hw`) |
| S6 startup | cold ≤ 800 ms, warm ≤ 400 ms (NFR-PERF-01) | `just bench-startup`: warm p50 ≈ 290–330 ms (#16, #18; summary in [27 §2](../27-delivery-plan.md#2-current-state-2026-10-07)) | pending, M0-S6b (`hw`) |
| Idle memory | ≤ 200 MB total footprint (NFR-PERF-02) | `just bench-memory`: ≈ 122–134 MB (#18) | pending, M0-S6b (`hw`) |

The repository does not record an M4 cold (first-launch) startup figure; M0-S6b measures the cold budget on mac-low. M0-09 tracks the ≈ 35 ms warm-start regression from #18.

Consequences confirmed so far:

- Rust drivers without native client libraries: the M0-S3 PostgreSQL prototype uses `postgres-protocol` ([05 §11](../05-database-drivers.md#11-m0-s3-postgresql-streaming-spike)) and the M0-S4 SSH spike uses `russh` ([24 §11](../24-networking-and-protocols.md#11-implementation-notes-m0-s4-ssh-spike)).
- The IPC boundary for large data works as designed in ADR-0002 (S1) and feeds the canvas grid (ADR-0004, S2).
- Only WebKit (WKWebView) has been measured. WebView2 and WebKitGTK are re-measured in M6-02.

If any mac-low run misses its budget, the owning task files an optimisation task first; this ADR is revisited only if the budget cannot be met.
