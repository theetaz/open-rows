// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect } from './dialects';

/** Placeholder syntax treated as query parameters (Settings ▸ Query Params). */
export type PlaceholderStyle = 'named' | 'numbered' | 'anonymous';

/** One parameter occurrence: its UTF-16 offset and the name its value is kept by. */
export type ParamOccurrence = { from: number; name: string };

type Kind = PlaceholderStyle | 'other';

/**
 * Sticky patterns tried in order, mirroring `openrows_sql::placeholders`:
 * comments, strings, quoted names and dollar quotes are skipped whole, so
 * look-alikes inside them are never parameters.
 */
function rules(dialect: EditorDialect): readonly [Kind, RegExp][] {
  const mysql = dialect === 'mysql' || dialect === 'mariadb';
  const sqlite = dialect === 'sqlite';
  return [
    ['other', mysql ? /\s+|--[^\n]*|#[^\n]*/y : /\s+|--[^\n]*/y],
    ['other', /\/\*[\s\S]*?(?:\*\/|$)/y],
    [
      'other',
      mysql
        ? /'(?:[^'\\]|\\[\s\S]|'')*'?|"(?:[^"\\]|\\[\s\S]|"")*"?/y
        : /[Ee]'(?:[^'\\]|\\[\s\S]|'')*'?|'(?:[^']|'')*'?|"(?:[^"]|"")*"?/y,
    ],
    ['other', /`(?:[^`]|``)*`?/y],
    ['other', /\$([A-Za-z_]\w*)?\$[\s\S]*?(?:\$\1\$|$)/y],
    ['other', /[A-Za-z_\u0080-￿][\w$\u0080-￿]*/y],
    ['numbered', sqlite ? /[$?]\d+/y : /\$\d+/y],
    [
      'named',
      sqlite
        ? /[:@$][A-Za-z_\u0080-￿][\w\u0080-￿]*/y
        : /:[A-Za-z_\u0080-￿][\w\u0080-￿]*/y,
    ],
    ['anonymous', /\?/y],
    ['other', /::+|:=|[\s\S]/y],
  ];
}

type Token = { kind: Kind; from: number; text: string };

function* tokens(sql: string, dialect: EditorDialect): Generator<Token> {
  const patterns = rules(dialect);
  let at = 0;
  while (at < sql.length) {
    for (const [kind, pattern] of patterns) {
      pattern.lastIndex = at;
      const text = pattern.exec(sql)?.[0];
      if (!text) continue;
      yield { kind, from: at, text };
      at += text.length;
      break;
    }
  }
}

/** `?|`, `?&` and `??` are operators, not parameters. */
function isOperator(sql: string, at: number): boolean {
  const near = (sql[at - 1] ?? '') + (sql[at + 1] ?? '');
  return /[|&?]/.test(near);
}

/**
 * Every parameter of `style` in `sql`, in source order. Named and numbered
 * parameters keep their text (`:id`, `$1`); anonymous `?` are numbered
 * `?1`, `?2`, … as the core asks for them. Brackets (array slices
 * `a[lo:hi]`) and `::` casts are never parameters.
 */
export function findParams(
  sql: string,
  dialect: EditorDialect,
  style: PlaceholderStyle,
): ParamOccurrence[] {
  const found: ParamOccurrence[] = [];
  let depth = 0;
  for (const token of tokens(sql, dialect)) {
    if (token.text === '[') depth += 1;
    else if (token.text === ']') depth = Math.max(0, depth - 1);
    if (token.kind !== style) continue;
    if (style === 'named' && depth > 0) continue;
    if (style === 'anonymous') {
      if (isOperator(sql, token.from)) continue;
      found.push({ from: token.from, name: `?${String(found.length + 1)}` });
    } else found.push({ from: token.from, name: token.text });
  }
  return found;
}

/** Distinct parameter names of `sql` in order of first use. */
export function paramNames(
  sql: string,
  dialect: EditorDialect,
  style: PlaceholderStyle,
): string[] {
  return [...new Set(findParams(sql, dialect, style).map(({ name }) => name))];
}
