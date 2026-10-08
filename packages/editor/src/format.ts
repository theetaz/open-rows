// SPDX-License-Identifier: Apache-2.0
import type { EditorView } from '@codemirror/view';
import type { EditorDialect } from './dialects';
import { currentStatement } from './statement';
import type { TextRange } from './statement';

/** How Beautify lays SQL out. */
export type FormatOptions = {
  dialect: EditorDialect;
  indentWidth: 2 | 4;
  indentWith: 'spaces' | 'tabs';
  keywordCase: 'preserve' | 'upper' | 'lower';
};

/** Beautify (pretty-print) or Uglify (one line). */
export type FormatKind = 'beautify' | 'uglify';

/**
 * Strings, quoted names, dollar quotes and block comments (kept verbatim),
 * line comments and whitespace runs; everything else is kept as written.
 */
const PIECES =
  /('(?:[^']|'')*'?|\/\*[\s\S]*?(?:\*\/|$)|"(?:[^"]|"")*"?|`(?:[^`]|``)*`?|\$([A-Za-z_]\w*)?\$[\s\S]*?(?:\$\2\$|$))|--([^\n]*)|(\s+)/g;

/**
 * Collapse SQL onto one line: whitespace runs become one space, line comments
 * become block comments (so they cannot swallow the rest), literals stay.
 */
export function uglifySql(sql: string): string {
  return sql
    .replace(
      PIECES,
      (piece, literal?: string, _tag?: string, comment?: string) => {
        if (literal) return piece;
        if (comment !== undefined)
          return `/* ${comment.trim().replaceAll('*/', '* /')} */`;
        return ' ';
      },
    )
    .trim();
}

/** Pretty-print SQL in the dialect; the formatter loads on first use. */
export async function beautifySql(
  sql: string,
  options: FormatOptions,
): Promise<string> {
  const { beautify } = await import('./beautify');
  return beautify(sql, options);
}

/** The selection, else the current statement, else the whole document. */
function formatTarget(view: EditorView): TextRange {
  const { state } = view;
  const { from, to, head } = state.selection.main;
  if (from !== to) return { from, to };
  return currentStatement(state, head) ?? { from: 0, to: state.doc.length };
}

/**
 * Beautify or uglify the selection, current statement or document as one
 * undoable change. Nothing changes when the text moved on meanwhile.
 *
 * @throws When the formatter cannot parse the SQL.
 */
export async function formatInView(
  view: EditorView,
  kind: FormatKind,
  options: FormatOptions,
): Promise<void> {
  const { from, to } = formatTarget(view);
  const before = view.state.sliceDoc(from, to);
  const text =
    kind === 'beautify'
      ? await beautifySql(before, options)
      : uglifySql(before);
  if (text === before || view.state.sliceDoc(from, to) !== before) return;
  view.dispatch({
    changes: { from, to, insert: text },
    selection: { anchor: from + text.length },
    userEvent: 'input.format',
    scrollIntoView: true,
  });
}
