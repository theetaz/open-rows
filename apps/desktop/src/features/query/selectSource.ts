// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';

type Token = { text: string; identifier: boolean; keyword: string };
type Source = { schema: string; table: string; columns: string[] | null };
const unsupported =
  'Only a single-table SELECT of plain columns is editable; joins, expressions, DISTINCT, grouping and set operations are read-only.';
const forbidden = new Set(
  'SELECT WITH JOIN UNION INTERSECT EXCEPT MINUS DISTINCT GROUP HAVING WINDOW QUALIFY INTO FOR LOCK RETURNING TABLESAMPLE LATERAL APPLY CURRENT_USER SESSION_USER SYSTEM_USER CURRENT_ROLE CURRENT_CATALOG CURRENT_SCHEMA CURRENT_DATE CURRENT_TIME CURRENT_TIMESTAMP LOCALTIME LOCALTIMESTAMP USER NULL TRUE FALSE'.split(
    ' ',
  ),
);
const clauses = new Set(['WHERE', 'ORDER', 'LIMIT', 'OFFSET', 'FETCH']);

/** Fail closed on syntax whose provenance cannot be proved by this small recognizer. */
export function selectSource(sql: string, engine: EngineDto): Source | string {
  if (
    ![
      'postgres',
      'mysql',
      'mariadb',
      'sqlite',
      'libsql',
      'libsql-file',
      'sqlserver',
      'duckdb',
    ].includes(engine)
  )
    return 'This engine does not expose safe editable SQL result mapping.';
  const tokens = tokenize(sql, engine);
  if (!tokens || tokens.shift()?.keyword !== 'SELECT') return unsupported;
  if (tokens.at(-1)?.text === ';') tokens.pop();
  if (
    tokens.some((token) => forbidden.has(token.keyword) || token.text === ';')
  )
    return unsupported;
  if (tokens[0]?.keyword === 'ALL') tokens.shift();
  const from = tokens.findIndex((token) => token.keyword === 'FROM');
  if (from <= 0) return unsupported;
  const table = fromTable(tokens.slice(from + 1));
  if (typeof table === 'string') return table;
  const columns = projection(tokens.slice(0, from), table);
  return columns === false
    ? unsupported
    : { schema: table.schema, table: table.table, columns };
}

type Table = { schema: string; table: string; alias: string };
function fromTable(tail: Token[]): Table | string {
  if (tail.some((token) => token.keyword === 'FROM')) return unsupported;
  const schema = tail.shift(),
    dot = tail.shift(),
    table = tail.shift();
  if (!schema?.identifier || dot?.text !== '.' || !table?.identifier)
    return 'Qualify the source table with its schema (for example public.users); the session search path is not available in result metadata.';
  if (tail[0]?.keyword === 'AS') tail.shift();
  let alias = table.text;
  const next = tail[0];
  if (next?.identifier && !clauses.has(next.keyword)) {
    alias = next.text;
    tail.shift();
  }
  const first = tail[0];
  if (first && !clauses.has(first.keyword)) return unsupported;
  return { schema: schema.text, table: table.text, alias };
}

function projection(tokens: Token[], table: Table): string[] | null | false {
  const groups: Token[][] = [[]];
  for (const token of tokens) {
    if (token.text === ',') groups.push([]);
    else groups.at(-1)?.push(token);
  }
  const names: string[] = [];
  for (const group of groups) {
    const name = directColumn(group, table);
    if (name === false) return false;
    if (name === '*') return groups.length === 1 ? null : false;
    names.push(name);
  }
  return names;
}
function directColumn(group: Token[], table: Table): string | false {
  if (qualifies(group, 5, table.schema)) group.splice(0, 2);
  if (qualifies(group, 3, table.alias)) group.splice(0, 2);
  if (group.length !== 1) return false;
  const column = group[0];
  if (column?.text === '*') return '*';
  return column?.identifier ? column.text : false;
}

/** Quotes/comments are tokenized, never removed with a SQL-shaped regular expression. */
function tokenize(sql: string, engine: EngineDto): Token[] | null {
  if (sql.length > 100_000 || sql.includes('\\') || sql.includes('\0'))
    return null;
  const pattern =
    /\s+|--[^\r\n]*|"(?:[^"]|"")*"|`(?:[^`]|``)*`|'(?:[^']|'')*'|[A-Za-z_][A-Za-z_0-9$]*|[0-9]+|[.,*;()=<>!+/%?:-]/gy;
  const tokens: Token[] = [];
  let at = 0;
  while (at < sql.length) {
    pattern.lastIndex = at;
    const match = pattern.exec(sql);
    if (!match) return null;
    const raw = match[0];
    at = pattern.lastIndex;
    if (/^\s|^--/.test(raw)) continue;
    const token = tokenOf(raw, engine);
    if (!token) return null;
    tokens.push(token);
  }
  return tokens;
}
function tokenOf(raw: string, engine: EngineDto): Token | null {
  const quote = raw.charAt(0);
  const quoted = quote === '"' || quote === '`';
  if (quote === '"' && ['mysql', 'mariadb'].includes(engine)) return null;
  if (
    quote === '`' &&
    !['mysql', 'mariadb', 'sqlite', 'libsql', 'libsql-file'].includes(engine)
  )
    return null;
  const word = /^[A-Za-z_]/.test(raw);
  const folded = word && engine === 'postgres';
  const text = quoted
    ? raw.slice(1, -1).replaceAll(quote.repeat(2), quote)
    : folded
      ? raw.toLowerCase()
      : raw;
  return {
    text,
    identifier: word || quoted,
    keyword: word ? raw.toUpperCase() : '',
  };
}

function qualifies(group: Token[], length: number, name: string): boolean {
  return (
    group.length === length && group[0]?.text === name && group[1]?.text === '.'
  );
}
