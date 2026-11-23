// SPDX-License-Identifier: Apache-2.0
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { tags } from '@lezer/highlight';

/**
 * Token colours come from `@openrows/ui` CSS custom properties so themes switch
 * through the root `data-theme` cascade without rebuilding editor state.
 * Syntax overrides remain plain colour tokens and never execute theme code.
 */
export const sqlHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: 'var(--or-syntax-keyword)' },
  {
    tag: [
      tags.typeName,
      tags.standard(tags.name),
      tags.function(tags.variableName),
    ],
    color: 'var(--or-syntax-function)',
  },
  {
    tag: [tags.string, tags.special(tags.string)],
    color: 'var(--or-syntax-string)',
  },
  {
    tag: [tags.number, tags.bool, tags.null],
    color: 'var(--or-syntax-number)',
  },
  {
    tag: [tags.lineComment, tags.blockComment],
    color: 'var(--or-syntax-comment)',
    fontStyle: 'italic',
  },
  {
    tag: [tags.operator, tags.punctuation, tags.bracket],
    color: 'var(--or-secondary)',
  },
  { tag: tags.special(tags.name), color: 'var(--or-danger-fg)' },
]);

/**
 * The v4 editor: 12 px mono on its host's fill (the query tab's
 * `--or-editor-fill` well), a tertiary gutter, an iris caret and a hover-tint
 * current line.
 */
const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--or-text)',
    backgroundColor: 'transparent',
    fontSize: 'var(--or-text-editor, var(--or-text-mono, var(--or-text-12)))',
  },
  '.cm-scroller': {
    fontFamily: 'var(--or-font-editor, var(--or-font-mono))',
    lineHeight: 'var(--or-editor-line-height, 1.5)',
  },
  '.cm-content': { caretColor: 'var(--or-accent)' },
  // drawSelection paints its own caret; without this it stays black in dark themes.
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'var(--or-accent)',
    borderLeftWidth: '1.5px',
  },
  '.cm-gutters': {
    color: 'var(--or-tertiary)',
    backgroundColor: 'transparent',
    border: 'none',
    fontSize: 'var(--or-text-11)',
  },
  '.cm-activeLine, .cm-activeLineGutter, .cm-currentQuery': {
    backgroundColor: 'var(--or-hover)',
  },
  // The accent selection tint keeps selected text readable in both themes.
  '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-content ::selection':
    {
      backgroundColor: 'var(--or-accent-soft) !important',
    },
});

/** Token-driven theme plus syntax highlighting for every SQL surface. */
export function editorAppearance(): Extension {
  return [editorTheme, syntaxHighlighting(sqlHighlightStyle)];
}
