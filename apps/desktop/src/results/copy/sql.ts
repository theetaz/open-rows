// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';

/** How a column's display text becomes a literal. */
export type ValueKind = 'number' | 'boolean' | 'text';

const NUMBER = /^-?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;
const BOOLEAN_TYPE = /^bool(ean)?$/i;

/** `true` when `text` can be written as a bare numeric literal. */
export function isNumberText(text: string): boolean {
  return NUMBER.test(text);
}

/** The literal kind of a database type: numeric (right-aligned), boolean or text. */
export function valueKind(type: string | null, numeric: boolean): ValueKind {
  if (numeric) return 'number';
  return type && BOOLEAN_TYPE.test(type) ? 'boolean' : 'text';
}

const mysql = (engine: EngineDto): boolean =>
  engine === 'mysql' || engine === 'mariadb';

/** One identifier quoted for the dialect (as `Dialect::quote_ident` in openrows-sql). */
export function quoteIdent(engine: EngineDto, name: string): string {
  if (engine === 'sqlserver') return `[${name.replaceAll(']', ']]')}]`;
  if (engine === 'clickhouse')
    return `\`${name.replaceAll('\\', '\\\\').replaceAll('`', '\\`')}\``;
  return mysql(engine)
    ? `\`${name.replaceAll('`', '``')}\``
    : `"${name.replaceAll('"', '""')}"`;
}

/** A table name for INSERT: schema-qualified on PostgreSQL and SQL Server, bare elsewhere. */
export function tableName(
  engine: EngineDto,
  table: { schema: string; name: string },
): string {
  const name = quoteIdent(engine, table.name);
  const qualified =
    engine === 'postgres' || engine === 'sqlserver' || engine === 'clickhouse';
  return qualified && table.schema
    ? `${quoteIdent(engine, table.schema)}.${name}`
    : name;
}

const MYSQL_ESCAPES: Readonly<Record<string, string>> = {
  "'": "\\'",
  '\\': '\\\\',
  '\0': '\\0',
  '\n': '\\n',
  '\r': '\\r',
  '\u001a': '\\Z',
};

/** A string literal; the rules match openrows-export's `push_string`. */
function stringLiteral(engine: EngineDto, text: string): string {
  if (engine === 'clickhouse')
    return `'${Array.from(text, (c) => (c === '\u001a' ? '\\x1a' : (MYSQL_ESCAPES[c] ?? c))).join('')}'`;
  if (mysql(engine))
    return `'${Array.from(text, (c) => MYSQL_ESCAPES[c] ?? c).join('')}'`;
  if (engine === 'sqlserver') return `N'${text.replaceAll("'", "''")}'`;
  if (engine === 'postgres' && text.includes('\\'))
    return `E'${text.replaceAll("'", "''").replaceAll('\\', '\\\\')}'`;
  return `'${text.replaceAll("'", "''")}'`;
}

/** A value as an SQL literal of the dialect; `null` is SQL NULL. */
export function sqlLiteral(
  engine: EngineDto,
  value: string | null,
  kind: ValueKind,
): string {
  if (value === null) return 'NULL';
  if (kind === 'number' && isNumberText(value)) return value;
  if (kind === 'boolean' && (value === 'true' || value === 'false')) {
    if (engine === 'sqlite' || engine === 'sqlserver')
      return value === 'true' ? '1' : '0';
    return value.toUpperCase();
  }
  return stringLiteral(engine, value);
}
