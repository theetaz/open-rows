# 0001 — Rust core engine with Tauri 2 shell and React UI

- Status: Accepted
- Date: 2026-10-05

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
