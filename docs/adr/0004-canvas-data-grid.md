# 0004 — Canvas-rendered data grid behind an adapter

- Status: Accepted
- Date: 2026-10-06 (Option A accepted); reviewed 2026-10-07 (M0-11)
- Task: M0-S2 / issue #22; NFR-PERF-04; docs/08; docs/13 §4

## Decision

Use our own Canvas 2D painter through `@openrows/grid/canvas`. The maintainer
accepted Option A after three release runs on a Mac mini M4, 60 Hz. Remove the
Glide adapter, dependencies, export and vendor instrumentation. Keep the shared
adapter, S1 binary tile client/Arrow worker and UI tokens.

## Native evidence (fix 2)

Numbers below are the maintainer's supplied measurements, not a new run. A dash
means the summary did not supply that metric; do not infer zero. Times are ms.

| Run | Renderer/scenario | p95 | p99 | max | >20 | >32 | Mean paint | Blank frames |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A | own/smooth | 17 | 18 | 25 | 1 | 0 | 1.89 | 0 |
| A | own/flings | 18 | 23 | 420 | 13 | 2 | 0.52 | 0 |
| A | own/horizontal | 18 | 22 | 52 | 5 | 1 | 0.65 | 0 |
| A | own/jump | — | — | 18 | — | 0 | — | — |
| A | Glide/smooth | — | — | 22 | — | 0 | — | — |
| A | Glide/flings | — | — | 348 | — | 3 | — | — |
| A | Glide/horizontal | — | — | 19 | — | — | — | — |
| A | Glide/jump | — | — | 28 | — | — | — | — |
| B | own/flings (page 113) | — | — | 75 | — | 1 | — | — |
| B | own/smooth, horizontal, jump | — | — | ≤32 each | — | 0 each | — | — |
| C | own/smooth (segment 0) | — | — | 630 | — | — | — | — |
| C | own/flings, horizontal, jump | — | — | ≤30 each | — | 0 each | — | — |
| C | Glide/smooth | — | — | 1746; another stall 632 | — | — | — | — |
| C | Glide/flings | — | — | 258 | — | — | — | — |

Own's measured complete paint is 0.5–1.9 ms/frame, with zero per-frame painter
allocations and zero blank-placeholder frames reported by the maintainer. Glide
run A also includes a 40 ms frame with a 39 ms React commit and 5120 cell
allocations. Its 348 ms fling stall had zero tracked work.

The large 258–1746 ms stalls appear at random steps in both renderers, with
scroll handler, commit, conversion, apply, resize, paint and allocation counters
all zero. The machine had unrelated heavy load (load average 6–8). The maintainer
classifies these as environmental stalls, not renderer cost. Attribution alone
does not identify OS scheduling, compositor or GC causes; no specific cause is
claimed without a profiler trace. Every raw interval remains in future reports.
Across the three own runs, each scenario has a repeat with zero frames >32 ms.

## Weighted criteria

Scores are engineering judgments from 1 (poor) to 5 (strong); High = 3 and
Medium = 2. The frame policy is a separate gate, not part of the weighted total.

| Criterion | Weight | A score / weighted | B score / weighted | Evidence and rationale |
| --- | --- | --- | --- | --- |
| Frames >32 ms | Gate | Met per scenario across A/B/C | Recorded above | Own smooth passes A/B, flings C, horizontal B/C, jump A/B/C; raw environmental stalls retained |
| Paint cost | 3 (High) | 5 / 15 | 3 / 9 | Own 0.52–1.89 ms, zero painter allocations; B has a 39 ms commit/5120 allocations; B mean paint was not supplied |
| Visual fit, pinning, multi-range | 3 (High) | 5 / 15 | 4 / 12 | Own directly controls edit-state visuals, pinned columns and disjoint ranges; B couples to vendor scroller/draw hooks |
| Accessibility hooks | 3 (High) | 3 / 9 | 3 / 9 | Same ARIA focused-cell shadow prototype; full screen-reader sign-off remains future work |
| Bundle size | 2 (Medium) | 5 / 10 | 2 / 4 | Own entry 2850 gzip bytes vs B 93912 plus 2012 CSS; both deferred |
| Code size | 2 (Medium) | 2 / 4 | 4 / 8 | Comparison source: A 459 renderer lines; B 367 adapter/timing lines plus 34-line build hook |
| Long-term maintenance | 2 (Medium) | 3 / 6 | 2 / 4 | A owns the implementation with full control; B adds React peer mismatch, dependencies and pinned private hooks |
| Total (15 weight units) | | **59 / 75** | **46 / 75** | Maintainer accepts A |

Historical sizes use `node:zlib.gzipSync` from the existing bundle gate. Entry
sizes exclude shared source, Arrow worker/client and initial React/IPC chunks.
B also emitted deferred overlay chunks of 1738 and 6044 gzip bytes. The spike's
initial JS was 95657 gzip bytes against a 350000-byte budget. Removal changes
chunk composition; `just bench-ui` reports current sizes.

## Dependency and licence record

The removed comparison used `@glideapps/glide-data-grid` 6.0.3 (MIT), plus its
required peers lodash 4.18.1, marked 4.3.0 and react-responsive-carousel 3.2.23
(all MIT). Its installed dependency/peer closure contained 87 packages; 16
contributed build modules, all MIT. Linaria tooling introduced caniuse-lite
(CC-BY-4.0) and tslib (0BSD), outside docs/22's shipped allowlist; neither entered
that build's shipped graph. This was evidence for the comparison, not a licence
exception. Glide/Linaria declared React peers through 18 while this workspace
used React 19.3.0. The mocked-canvas real-library mount passed without establishing
native compatibility. These dependencies and the dedicated licence checker are
now removed; Option A adds no external dependency.

## Consequences and gate

`just bench-grid [repeats=3]` measures only the accepted renderer. It runs all
four scenarios in a fresh app launch for each repeat, with fresh mounts, zero
scroll position and fresh tile caches. For each scenario the best repeat must
have **zero frames strictly >32 ms**. Independently, **every repeat** must have
zero frames >32 ms whose inclusive tracked main-thread timing sum is **≥16 ms**.
Worker CPU is excluded; overlapping inclusive categories make this a conservative
diagnostic, not exact CPU accounting. All repeats, raw samples and attribution
are printed/saved. Missing or invalid samples fail closed. See
[docs/13 §4](../13-performance.md#4-benchmark-suite) for the policy and workload.

The retained prototype has four canvas layers, bounded text/tile caches, pinned
columns, edit-state token visuals, disjoint selections and an ARIA shadow grid.
Production editing, copy, selection gestures, full keyboard behavior, neighbouring
ARIA rows and screen-reader testing remain future work. Accepting the renderer
and S2 spike does not certify the complete production grid or other reference
hardware. There is no main performance baseline.

## How to reproduce the comparison

Use this PR's git history **before the fix-3 removal** (the fix-2 comparison
revision), which contains both adapters and the pinned dependency lockfile.
At that revision, run `just build-app` then `just bench-grid` on the reference
Mac; it prints both options. The Glide adapter is intentionally absent from the
accepted revision. At the current revision, `just check` tests gates without a
GUI; `just build-app` then `just bench-grid 3` measures the retained painter.

## Validation / evidence — 2026-10-07 (M0-11 review)

Accepted with evidence; no revision. The M0-S2 frame gate (best repeat per scenario: zero frames > 32 ms) is met on the maintainer's M4/60 Hz runs above (#23). The S2 acceptance covers the renderer only. The production grid, editing, copy and screen-reader testing are M1-19 (`just bench-grid` must still pass there). Runs on mac-low and on other webviews are revisit points: X-02 and M1-32 for mac-low, M6-02 for WebView2 and WebKitGTK.
