# 0004 — Canvas-rendered data grid behind an adapter

- Status: Proposed (decision between options A and B in M0 spike S2)
- Date: 2026-10-05

## Context

The grid must scroll at display refresh rate with 1 M+ rows and 100+ columns, render rich cell states, and support Excel-like selection and editing. DOM-virtualised grids struggle in WebKit webviews at this density due to style/layout cost per cell node.

## Decision

Render the cell area with **Canvas 2D**, keep DOM for overlays (editor, menus, tooltips) and a shadow ARIA grid for accessibility. Hide the implementation behind the `@openrows/grid` adapter API so the rest of the app is independent of the choice below.

- **Option A — Own renderer:** custom painter (layers for header/body/selection/pinned), our tile cache, our input controller.
- **Option B — Glide Data Grid (MIT):** proven canvas grid, adapted through our adapter; contribute fixes upstream or maintain a fork if needed.

## Decision criteria for S2

| Criterion | Weight |
| --- | --- |
| Frames > 32 ms during scripted 1 M-row scroll (must be 0) | Gate |
| Paint cost per frame at 60×20 visible cells | High |
| Fit for our edit-state visuals, pinned columns, multi-range selection | High |
| Accessibility hooks | High |
| Code size / bundle size | Medium |
| Long-term maintenance cost | Medium |

## Consequences

- ✅ Smooth scrolling and low memory regardless of row count.
- ⚠️ Accessibility needs deliberate work (shadow grid, live regions).
- ⚠️ Text rendering must be carefully tuned for crispness at all DPRs.

## Alternatives considered

- DOM virtualised grids (AG Grid community, TanStack Table + virtualiser): excellent APIs, but per-cell DOM cost is the bottleneck we are avoiding. TanStack Table may still be used for small, non-performance-critical tables (e.g. settings lists).
