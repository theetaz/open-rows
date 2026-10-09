// SPDX-License-Identifier: Apache-2.0
import { EditorView } from '@codemirror/view';

/** Ten 24 px rows plus the list padding. */
const ROW = '24px';
const LIST_HEIGHT = '248px';

const tooltip = '& .cm-tooltip.cm-tooltip-autocomplete';
const row = `${tooltip} > ul > li`;
/** Wins over CodeMirror's `&light`/`&dark` base colours for the selection. */
const selected = `${row}[aria-selected]`;

/**
 * Completion popup in `@openrows/ui` tokens, so light and dark themes follow
 * the root `data-theme` cascade: panel surface, separator ring, UI font,
 * accent selection with on-accent text and per-kind icon tints.
 */
export const completionTheme = EditorView.theme({
  '& .cm-tooltip': {
    color: 'var(--or-text)',
    backgroundColor: 'var(--or-panel-solid)',
    border: 'var(--or-border-width) solid var(--or-separator)',
    borderRadius: 'var(--or-radius-field)',
    boxShadow: 'var(--or-panel-shadow)',
  },
  [tooltip]: { overflow: 'hidden' },
  [`${tooltip} > ul`]: {
    fontFamily: 'var(--or-font-ui)',
    fontSize: 'var(--or-text-13)',
    maxHeight: LIST_HEIGHT,
    minWidth: '280px',
    padding: 'var(--or-space-4)',
    boxSizing: 'border-box',
  },
  [row]: {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--or-space-6)',
    height: ROW,
    lineHeight: ROW,
    padding: '0 var(--or-space-8)',
    borderRadius: 'var(--or-radius-field)',
    color: 'var(--or-text)',
  },
  [selected]: {
    backgroundColor: 'var(--or-accent)',
    color: 'var(--or-on-accent)',
  },
  '& .cm-completionLabel': {
    flex: '1 1 auto',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  '& .cm-completionMatchedText': { textDecoration: 'none' },
  '& .cm-completionDetail': {
    marginLeft: 'auto',
    paddingLeft: 'var(--or-space-12)',
    fontStyle: 'normal',
    fontSize: 'var(--or-text-12)',
    color: 'var(--or-secondary)',
  },
  [`${selected} .cm-completionDetail, ${selected} .cm-sqlIcon`]: {
    color: 'var(--or-on-accent)',
  },
  '& .cm-sqlIcon': {
    flex: 'none',
    width: '14px',
    height: '14px',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: '1.8',
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
  },
  '& .cm-sqlIcon-keyword': { color: 'var(--or-secondary)' },
  '& .cm-sqlIcon-table': { color: 'var(--or-info-fg)' },
  '& .cm-sqlIcon-view': { color: 'var(--or-success-fg)' },
  '& .cm-sqlIcon-function': { color: 'var(--or-warning-fg)' },
  '& .cm-sqlIcon-column': { color: 'var(--or-accent)' },
  '& .cm-sqlIcon-schema': { color: 'var(--or-danger-fg)' },
  '& .cm-sqlIcon-snippet': { color: 'var(--or-secondary)' },
});
