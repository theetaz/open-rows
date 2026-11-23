// SPDX-License-Identifier: Apache-2.0
/**
 * Styles of the diagram SVG, embedded in it so a standalone export keeps
 * them; colours are theme tokens, resolved to values when exporting. Class
 * names are prefixed because an inline `<style>` applies document-wide.
 */
export const DIAGRAM_STYLE = `
.erd-box { fill: var(--or-group); stroke: none; }
.erd-head { fill: var(--or-control-fill); }
.erd-rule { stroke: none; }
.erd-table text { font-family: var(--or-font-mono); font-size: 12px; fill: var(--or-text); }
.erd-table .erd-title { font-family: var(--or-font-ui); font-weight: 600; }
.erd-table .erd-pk { font-weight: 600; }
.erd-table .erd-fk { fill: var(--or-accent); }
.erd-table .erd-type { fill: var(--or-secondary); }
.erd-table .erd-key { fill: var(--or-secondary); font-family: var(--or-font-ui); font-size: 9.5px; font-weight: 700; }
.erd-table .erd-ref { fill: var(--or-accent); font-family: var(--or-font-ui); font-size: 9.5px; font-weight: 700; }
.erd-table .erd-more { fill: var(--or-secondary); font-family: var(--or-font-ui); font-style: italic; }
.erd-edge { fill: none; stroke: var(--or-separator-strong); stroke-width: 1.25; }
.erd-mark { fill: none; stroke: var(--or-separator-strong); stroke-width: 1.25; }
.erd-table[data-match] .erd-box { stroke: var(--or-accent); stroke-width: 2; }
.erd-table:focus { outline: none; }
.erd-table:focus-visible .erd-box { stroke: var(--or-focus); stroke-width: 2; }
[data-searching] .erd-table:not([data-match]) { opacity: 0.35; }
[data-searching] .erd-edge { opacity: 0.35; }
`;
