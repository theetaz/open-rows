# 0003 — CodeMirror 6 for all code editors

- Status: Accepted — validation pending (M0-S5: maintainer `just bench-editor` run, manual IME check, mac-low run)
- Date: 2026-10-05; evidence 2026-10-07; reviewed 2026-10-07 (M0-11)
- Task: M0-S5; NFR-PERF-07, FR-EDIT-02, FR-EDIT-09; docs/09; docs/13 §4

## Context

We need editors for SQL dialects, MQL, Redis commands, JSON, GraphQL, JavaScript (scripts) and YAML, running inside WKWebView/WebView2/WebKitGTK with low memory and excellent typing latency, plus Vim mode and IME support.

## Decision

Use **CodeMirror 6** for every code-editing surface, wrapped in `@openrows/editor`.

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

## M0-S5 evidence — 2026-10-07

Workload, metric definition and fail-closed rules: [13 §4, M0 S5](../13-performance.md#m0-s5-editor-keystroke-measurement).
In short: release app, `just bench-editor`, 5 000-line deterministic PostgreSQL document,
240 keystrokes per configuration at the top, middle and end (60 typed through WebKit's
native `insertText` path, 20 deletions via `keydown`). Keystroke-to-paint runs from
the scheduled keystroke to after the rendering update of the first frame following
CodeMirror's update. It excludes only the idle wait for that frame.

Measured by the S5 implementer (not the maintainer) on a Mac mini M4, macOS 26.5,
60 Hz display; page timer 1.0 ms. Four valid runs (build details in docs/13), all ms:

| Configuration | p95 (runs 1/2/3/4) | max (runs 1/2/3/4) | Budget | Result |
| --- | --- | --- | --- | --- |
| PostgreSQL highlighting + completion | 11 / 9 / 10 / 9 | 32 / 30 / 31 / 30 | p95 ≤ 16 | pass |
| Same + Vim mode | 9 / 9 / 9 / 10 | 20 / 23 / 26 / 20 | p95 ≤ 16 | pass |

Wall latency including the vsync wait had p95 ≤ 17 ms in every run. The tail is
the `'` key on line 2: opening or closing a string near the top re-highlights the
rest of the document, costing 18–26 ms of input handling. Bundle impact: the
initial JS is 95,222 gzip bytes of 350,000. CodeMirror + PostgreSQL is ≈ 123 KB gzip and Vim
≈ 41 KB gzip, both in lazy chunks and denied in the initial graph.

Dependencies (all MIT): `@codemirror/{state,view,language,commands,autocomplete,lang-sql,search}`,
`@lezer/highlight`, `@replit/codemirror-vim`. Transitive: `@lezer/{common,lr}`,
`@marijn/find-cluster-break`, `@replit/codemirror-vim-core`, `crelt`, `style-mod`,
`w3c-keyname`.

Still open:

- **IME (manual, `hw`).** Run the manual IME check (macOS Japanese – Romaji and Chinese
  Pinyin input sources) from the S5 PR's "How to try it". Record the outcome here.
- **Maintainer confirmation.** Run `just build-app` and `just bench-editor` on an idle M4, with the window
  frontmost. The final build's own runs were rejected because the window lost focus
  during interactive use (see docs/13).
- **mac-low.** Re-run `just bench-editor` on the reference low-end Mac.

## Validation / evidence — 2026-10-07 (M0-11 review)

Status stays pending. The repository holds only the implementer's four M4 runs above; none of the three open items has a recorded result. They all belong to M0-S5 (status `hw` in [27 §3](../27-delivery-plan.md#3-m0--foundations-and-spikes-remaining)). Steps that move this ADR to plain `Accepted`:

1. Maintainer: `just build-app`, then `just bench-editor` on an idle M4 with the window frontmost. Pass: p95 ≤ 16 ms for both `sql` and `vim`. Record the run table here and in [13 §4](../13-performance.md#m0-s5-editor-keystroke-measurement).
2. Manual IME check, as in the S5 PR (#27) "How to try it": `just dev`, **Component gallery** → **SQL editor**. Japanese – Romaji: type `nihongo` at the end of line 1, Space cycles candidates, Return commits 日本語. Pinyin – Simplified: type `zhongwen` then `1`, giving 中文. Pass: nothing is lost or doubled and highlighting continues afterwards; the same holds in Vim insert mode. Record the outcome here.
3. mac-low: `just bench-editor` on the reference low-end Mac. Pass: the same p95 budget. Record it here and in docs/13.

If step 1 or 3 misses the budget, profile the `'` re-highlight tail first (see above; M1-21 keeps watching it). If step 2 fails, record the failure here and revise this ADR explicitly before M1-21 builds on it.
