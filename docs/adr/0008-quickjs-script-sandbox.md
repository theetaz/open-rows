# 0008 — QuickJS sandbox for API scripts

- Status: Accepted — validation deferred (M4-05)
- Date: 2026-10-05; reviewed 2026-10-07 (M0-11)

## Context

API collections need pre-request/post-response scripts and tests in JavaScript (users expect JS; Postman compatibility). Collections come from git repos and may be untrusted. Scripts must be isolated from the filesystem, processes and the webview, and bounded in time and memory.

## Decision

Run scripts in **QuickJS** embedded via `rquickjs` inside the Rust core, one fresh context per run, with memory limit, interrupt-based time limit, and only a curated `ors` (and `pm` compatibility) API exposed. Crypto and other helpers are implemented in Rust and exposed as functions.

## Consequences

- ✅ Small footprint (sub-MB), fast startup per context, strong isolation boundary we control.
- ✅ Same engine in app and CLI → identical behaviour in CI.
- ⚠️ No JIT: heavy scripts are slower than V8 — acceptable for request scripting.
- ⚠️ No npm packages in scripts in 1.0; curated helpers instead.

## Alternatives considered

- **Running scripts in the webview:** fast to build but mixes untrusted code with the privileged UI; rejected.
- **V8 (`rusty_v8`) / Deno core:** fast, but tens of MB and heavier build.
- **Boa (pure Rust JS):** promising, currently slower and less complete than QuickJS.

## Validation / evidence — 2026-10-07 (M0-11 review)

Kept as Accepted rather than moved back to Proposed: it is a design decision that [03](../03-architecture-overview.md) and [11](../11-api-client.md) already build on, and no code depends on it before M4. There is no M0 spike. Validation is deferred to **M4-05** (`openrows-script`): one fresh context per run, memory and interrupt-based time limits, only the `ors`/`pm` API exposed, and restricted mode for untrusted collections, each covered by tests. M5-08 (collection runner and CLI) then confirms the CLI runs the same engine. If M4-05 cannot enforce these limits with `rquickjs`, this ADR is revised explicitly before scripts ship.
