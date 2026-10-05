# 0008 — QuickJS sandbox for API scripts

- Status: Accepted
- Date: 2026-10-05

## Context

API collections need pre-request/post-response scripts and tests in JavaScript (users expect JS; Postman compatibility). Collections come from git repos and may be untrusted. Scripts must be isolated from the filesystem, processes and the webview, and bounded in time and memory.

## Decision

Run scripts in **QuickJS** embedded via `rquickjs` inside the Rust core, one fresh context per run, with memory limit, interrupt-based time limit, and only a curated `odb` (and `pm` compatibility) API exposed. Crypto and other helpers are implemented in Rust and exposed as functions.

## Consequences

- ✅ Small footprint (sub-MB), fast startup per context, strong isolation boundary we control.
- ✅ Same engine in app and CLI → identical behaviour in CI.
- ⚠️ No JIT: heavy scripts are slower than V8 — acceptable for request scripting.
- ⚠️ No npm packages in scripts in 1.0; curated helpers instead.

## Alternatives considered

- **Running scripts in the webview:** fast to build but mixes untrusted code with the privileged UI; rejected.
- **V8 (`rusty_v8`) / Deno core:** fast, but tens of MB and heavier build.
- **Boa (pure Rust JS):** promising, currently slower and less complete than QuickJS.
