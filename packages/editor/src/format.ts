// SPDX-License-Identifier: Apache-2.0
import type { EditorView } from '@codemirror/view';
import type { EditorDialect } from './dialects';
import type { TextRange } from './statement';

/** How Beautify lays SQL out. */
export type FormatOptions = {
  dialect: EditorDialect;
  indentWidth: 2 | 4;
  indentWith: 'spaces' | 'tabs';
  keywordCase: 'preserve' | 'upper' | 'lower';
  /** Standard indentation, or keywords in a column aligned left or right (sql-formatter `indentStyle`). */
  indentStyle: 'standard' | 'tabularLeft' | 'tabularRight';
  /** Empty lines between statements. */
  linesBetweenQueries: 1 | 2 | 3;
  /** Operators without surrounding spaces (`a=1`), as sql-formatter's `denseOperators`. */
  denseOperators: boolean;
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

/** The selection, else the whole document. */
function formatTarget(view: EditorView): TextRange {
  const { from, to } = view.state.selection.main;
  return from !== to ? { from, to } : { from: 0, to: view.state.doc.length };
}

/**
 * Beautify or uglify the selection, else the whole document, as one undoable
 * change. Nothing changes when the text moved on meanwhile.
 *
 * @throws When the formatter cannot parse the SQL.
 */
export async function formatInView(
  view: EditorView,
  kind: FormatKind,
  options: FormatOptions,
): Promise<void> {
  if (options.dialect === 'mongodb')
    throw new Error(
      'Use the Pipeline or document editor to format MongoDB JSON.',
    );
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
