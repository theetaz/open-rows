// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { isNumberText, quoteIdent, sqlLiteral } from './sql';
import type { ValueKind } from './sql';
import type { DelimitedOptions } from './copySettings';
import { htmlTable } from './html';

/** What a grid selection can be copied as (FR-GRID-03). */
export type CopyFormat =
  'tsv' | 'csv' | 'json' | 'markdown' | 'html' | 'insert' | 'whereIn';

/** A copied column: its name and how its values become literals. */
export type CopyColumn = { name: string; kind: ValueKind };

/** Selected rows × columns as display text; `null` is SQL NULL. */
export type CopyTable = {
  columns: readonly CopyColumn[];
  rows: readonly (readonly (string | null)[])[];
};

/** Dialect and quoted target table for the SQL formats. */
type SqlTarget = { engine: EngineDto; table: string };

/** A row of inspector fields as one TSV line; hidden identity fields are left out by the caller. */
export function rowText(
  fields: readonly { text: string; state: string }[],
): string {
  return delimited(
    {
      columns: [],
      rows: [
        fields.map((field) =>
          field.state === 'null'
            ? null
            : field.state === 'empty'
              ? ''
              : field.text,
        ),
      ],
    },
    TSV_DEFAULTS,
  );
}

/** Menu labels of the `Copy as` formats, in TablePlus order. */
export const COPY_AS: readonly { format: CopyFormat; label: string }[] = [
  { format: 'csv', label: 'CSV' },
  { format: 'json', label: 'JSON' },
  { format: 'markdown', label: 'Markdown' },
  { format: 'html', label: 'HTML table' },
  { format: 'insert', label: 'SQL INSERT' },
  { format: 'whereIn', label: 'WHERE IN' },
];

const TSV_DEFAULTS: DelimitedOptions = {
  headers: false,
  nulls: 'text',
  quote: 'minimal',
  delimiter: '\t',
  quoteChar: '"',
  newline: '\n',
  decimal: '.',
};
const CSV_DEFAULTS: DelimitedOptions = {
  ...TSV_DEFAULTS,
  headers: true,
  nulls: 'empty',
  delimiter: ',',
};

/** A numeric column's value with the chosen decimal separator. */
function withDecimal(
  value: string,
  kind: ValueKind | undefined,
  decimal: DelimitedOptions['decimal'],
): string {
  if (decimal === '.' || kind !== 'number' || !isNumberText(value))
    return value;
  return value.replace('.', decimal);
}

/**
 * Delimited text: a header record when asked, NULL as `NULL` or an empty
 * field, numbers with the chosen decimal separator, and values quoted
 * (quotes doubled) when they need it or always; records end with the
 * chosen newline.
 */
function delimited(table: CopyTable, options: DelimitedOptions): string {
  const { delimiter, quoteChar: q } = options;
  const field = (value: string | null, kind?: ValueKind): string => {
    const text =
      value === null
        ? options.nulls === 'text'
          ? 'NULL'
          : ''
        : withDecimal(value, kind, options.decimal);
    const special =
      text.includes(delimiter) || text.includes(q) || /[\n\r]/.test(text);
    const quote = special || (options.quote === 'all' && value !== null);
    return quote ? `${q}${text.replaceAll(q, q + q)}${q}` : text;
  };
  const lines = table.rows.map((row) =>
    row
      .map((value, index) => field(value, table.columns[index]?.kind))
      .join(delimiter),
  );
  if (!options.headers) return lines.join(options.newline);
  const header = table.columns.map((column) => field(column.name));
  return [header.join(delimiter), ...lines].join(options.newline);
}

/** A JSON value; numeric text stays exact (no float round trip). */
function jsonValue(value: string | null, kind: ValueKind): string {
  if (value === null) return 'null';
  if (kind === 'number' && isNumberText(value) && !/^-?0\d/.test(value))
    return value.replace(/^(-?)\./, '$10.').replace(/\.(?=$|[eE])/, '');
  if (kind === 'boolean' && (value === 'true' || value === 'false'))
    return value;
  return JSON.stringify(value);
}

/** An array of row objects keyed by column name, two-space indented. */
function json(table: CopyTable): string {
  if (!table.rows.length) return '[]';
  const objects = table.rows.map((row) => {
    const fields = table.columns.map(
      (column, index) =>
        `    ${JSON.stringify(column.name)}: ${jsonValue(row[index] ?? null, column.kind)}`,
    );
    return `  {\n${fields.join(',\n')}\n  }`;
  });
  return `[\n${objects.join(',\n')}\n]`;
}

/** A GitHub-flavoured Markdown table; numbers right-aligned. */
function markdown(table: CopyTable): string {
  const cell = (value: string | null): string =>
    (value ?? 'NULL')
      .replaceAll('\\', '\\\\')
      .replaceAll('|', '\\|')
      .replace(/\r?\n|\r/g, '<br>');
  const line = (cells: readonly string[]): string => `| ${cells.join(' | ')} |`;
  return [
    line(table.columns.map((column) => cell(column.name))),
    line(
      table.columns.map((column) =>
        column.kind === 'number' ? '---:' : '---',
      ),
    ),
    ...table.rows.map((row) => line(row.map(cell))),
  ].join('\n');
}

/** One `INSERT … VALUES (…);` statement per row. */
function insert(table: CopyTable, target: SqlTarget): string {
  const names = table.columns
    .map((column) => quoteIdent(target.engine, column.name))
    .join(', ');
  return table.rows
    .map((row) => {
      const values = table.columns.map((column, index) =>
        sqlLiteral(target.engine, row[index] ?? null, column.kind),
      );
      return `INSERT INTO ${target.table} (${names}) VALUES (${values.join(', ')});`;
    })
    .join('\n');
}

/** `col IN (…)` of one column's distinct values, with NULL as `IS NULL`. */
function inPredicate(
  table: CopyTable,
  index: number,
  engine: EngineDto,
): string {
  const column = table.columns[index];
  if (!column) return '';
  const name = quoteIdent(engine, column.name);
  const values = new Set<string>();
  let nulls = false;
  for (const row of table.rows) {
    const value = row[index] ?? null;
    if (value === null) nulls = true;
    else values.add(sqlLiteral(engine, value, column.kind));
  }
  if (!values.size) return `${name} IS NULL`;
  const list = `${name} IN (${[...values].join(', ')})`;
  return nulls ? `(${list} OR ${name} IS NULL)` : list;
}

/** `WHERE a IN (…) AND b IN (…)` over every selected column. */
function whereIn(table: CopyTable, target: SqlTarget): string {
  const predicates = table.columns.map((_, index) =>
    inPredicate(table, index, target.engine),
  );
  return `WHERE ${predicates.join('\nAND ')}`;
}

/** Text of `table` in `format`; SQL formats use the target's dialect. */
export function formatTable(
  format: CopyFormat,
  table: CopyTable,
  target: SqlTarget,
  options?: DelimitedOptions,
): string {
  switch (format) {
    case 'tsv':
      return delimited(table, options ?? TSV_DEFAULTS);
    case 'csv':
      return delimited(table, options ?? CSV_DEFAULTS);
    case 'html':
      return htmlTable(table, options?.nulls ?? 'text');
    case 'json':
      return json(table);
    case 'markdown':
      return markdown(table);
    case 'insert':
      return insert(table, target);
    case 'whereIn':
      return whereIn(table, target);
  }
}
