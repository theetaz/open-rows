# 0003 — CodeMirror 6 for all code editors

- Status: Accepted
- Date: 2026-10-05

## Context

We need editors for SQL dialects, MQL, Redis commands, JSON, GraphQL, JavaScript (scripts) and YAML, running inside WKWebView/WebView2/WebKitGTK with low memory and excellent typing latency, plus Vim mode and IME support.

## Decision

Use **CodeMirror 6** for every code-editing surface, wrapped in `@odb/editor`.

## Consequences

- ✅ Modular, small bundles; only needed languages load.
- ✅ Strong large-document performance and accessibility; good behaviour in WebKit.
- ✅ Lezer grammars let us write custom languages (MQL, Redis) cleanly.
- ✅ Mature Vim keybinding extension.
- ⚠️ Fewer batteries than Monaco (no built-in LSP client, fewer out-of-box language services) — we build completion sources ourselves, which we need anyway for schema-aware completion.

## Alternatives considered

- **Monaco:** feature-rich but heavy (multi-MB, workers), tuned for Chromium/VS Code, less pleasant in WebKit and for IME.
- **Ace:** older architecture, less active.

## Validation

M0 spike S5: ≤ 16 ms keystroke p95 on 5 000-line SQL in WKWebView with highlighting and completion active.
