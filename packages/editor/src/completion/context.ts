// SPDX-License-Identifier: Apache-2.0
import { isName, isWord } from './lexer';
import type { SqlToken } from './lexer';

/** A relation the statement reads or writes, as written. */
export type TableRef = {
  schema: string | null;
  name: string;
  alias: string | null;
};

/**
 * What the word at the cursor can be: statement start (keywords), a relation
 * after FROM/JOIN/UPDATE/INTO/TABLE, a member of `path.` (columns or schema
 * objects), or an expression (columns, functions, keywords).
 */
export type CompletionScope =
  | { kind: 'start' }
  | { kind: 'relation' }
  | { kind: 'member'; path: readonly string[] }
  | { kind: 'expression' };

/** The scope at the cursor and every relation the statement references. */
export type StatementContext = {
  scope: CompletionScope;
  references: readonly TableRef[];
};

const RELATION_WORDS = new Set(['from', 'join', 'update', 'into', 'table']);

const EXPRESSION_WORDS = new Set([
  'select',
  'where',
  'on',
  'by',
  'having',
  'set',
  'and',
  'or',
  'not',
  'when',
  'then',
  'else',
  'case',
  'values',
  'returning',
  'distinct',
  'in',
  'like',
  'between',
  'is',
]);

/** Words that end a relation reference instead of naming its alias. */
const CLAUSE_WORDS = new Set([
  ...RELATION_WORDS,
  ...EXPRESSION_WORDS,
  'as',
  'inner',
  'left',
  'right',
  'full',
  'outer',
  'cross',
  'natural',
  'using',
  'group',
  'order',
  'limit',
  'offset',
  'union',
  'intersect',
  'except',
  'window',
  'fetch',
  'for',
  'lateral',
  'with',
  'default',
]);

const wordIn = (set: ReadonlySet<string>, token: SqlToken | undefined) =>
  token?.type === 'word' && set.has(token.value.toLowerCase());

/** A dotted name from `start`, e.g. `public.users`, and the index after it. */
function pathAt(
  tokens: readonly SqlToken[],
  start: number,
): { path: string[]; next: number } {
  const path = [tokens[start]?.value ?? ''];
  let at = start + 1;
  while (tokens[at]?.value === '.' && isName(tokens[at + 1])) {
    path.push(tokens[at + 1]?.value ?? '');
    at += 2;
  }
  return { path, next: at };
}

/** An optional `[AS] alias` at `at`. */
function aliasAt(
  tokens: readonly SqlToken[],
  at: number,
): { alias: string | null; next: number } {
  const start = isWord(tokens[at], 'as') ? at + 1 : at;
  const candidate = tokens[start];
  if (isName(candidate) && !wordIn(CLAUSE_WORDS, candidate))
    return { alias: candidate.value, next: start + 1 };
  return { alias: null, next: start };
}

function referenceAt(
  tokens: readonly SqlToken[],
  start: number,
): { ref: TableRef; next: number } | null {
  const first = tokens[start];
  if (!isName(first) || wordIn(CLAUSE_WORDS, first)) return null;
  const { path, next } = pathAt(tokens, start);
  // `schema.` still being typed names no relation yet.
  if (tokens[next]?.value === '.') return null;
  const { alias, next: after } = aliasAt(tokens, next);
  const name = path[path.length - 1] ?? '';
  const schema = path.length > 1 ? (path[path.length - 2] ?? null) : null;
  return { ref: { schema, name, alias }, next: after };
}

/** Relations named after FROM (comma lists included), JOIN, UPDATE, INTO, TABLE. */
function tableReferences(tokens: readonly SqlToken[]): TableRef[] {
  const found: TableRef[] = [];
  tokens.forEach((token, index) => {
    if (!wordIn(RELATION_WORDS, token)) return;
    let at = index + 1;
    for (;;) {
      const read = referenceAt(tokens, at);
      if (!read) return;
      found.push(read.ref);
      if (!isWord(token, 'from') || tokens[read.next]?.value !== ',') return;
      at = read.next + 1;
    }
  });
  return found;
}

/** Names before a trailing `.`, e.g. `public.users.` → `['public', 'users']`. */
function qualifier(before: readonly SqlToken[]): string[] {
  const path: string[] = [];
  let at = before.length - 1;
  while (before[at]?.value === '.' && isName(before[at - 1])) {
    path.unshift(before[at - 1]?.value ?? '');
    at -= 2;
  }
  return path;
}

/** The clause the cursor is in, judged by the nearest clause keyword. */
function clauseOf(
  before: readonly SqlToken[],
): 'relation' | 'expression' | null {
  for (let at = before.length - 1; at >= 0; at -= 1) {
    const token = before[at];
    if (wordIn(RELATION_WORDS, token)) return 'relation';
    if (wordIn(EXPRESSION_WORDS, token) || token?.value === '(')
      return 'expression';
  }
  return null;
}

function scopeOf(before: readonly SqlToken[]): CompletionScope {
  const last = before[before.length - 1];
  if (!last) return { kind: 'start' };
  if (last.value === '.') return { kind: 'member', path: qualifier(before) };
  if (wordIn(RELATION_WORDS, last)) return { kind: 'relation' };
  const clause = clauseOf(before);
  if (clause === 'relation')
    return last.value === ',' ? { kind: 'relation' } : { kind: 'start' };
  return clause ? { kind: 'expression' } : { kind: 'start' };
}

/**
 * Analyse one statement's tokens for the word starting at `wordFrom`: what
 * may be typed there, and which relations the whole statement references
 * (also those after the cursor, as in `SELECT | FROM users`).
 */
export function analyzeStatement(
  tokens: readonly SqlToken[],
  wordFrom: number,
): StatementContext {
  const before = tokens.filter((token) => token.to <= wordFrom);
  return { scope: scopeOf(before), references: tableReferences(tokens) };
}
