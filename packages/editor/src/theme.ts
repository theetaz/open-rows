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
  { tag: tags.keyword, color: 'var(--or-syntax-keyword)', fontWeight: '600' },
  {
    tag: [tags.typeName, tags.standard(tags.name)],
    color: 'var(--or-info-fg)',
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

const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--or-text)',
    backgroundColor: 'var(--or-page)',
    fontSize: 'var(--or-text-editor, var(--or-text-mono, var(--or-text-13)))',
  },
  '.cm-scroller': {
    fontFamily: 'var(--or-font-editor, var(--or-font-mono))',
    lineHeight: 'var(--or-editor-line-height, 1.5)',
  },
  '.cm-content': { caretColor: 'var(--or-accent)' },
  '.cm-gutters': {
    color: 'var(--or-tertiary)',
    backgroundColor: 'var(--or-page)',
    borderRight: 'var(--or-border-width) solid var(--or-separator)',
  },
  '.cm-activeLine, .cm-activeLineGutter, .cm-currentQuery': {
    backgroundColor: 'var(--or-row-stripe)',
  },
});

/** Token-driven theme plus syntax highlighting for every SQL surface. */
export function editorAppearance(): Extension {
  return [editorTheme, syntaxHighlighting(sqlHighlightStyle)];
}
