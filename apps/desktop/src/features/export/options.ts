// SPDX-License-Identifier: Apache-2.0
import type { CsvDelimiterDto, ExportFormatDto } from '@openrows/ipc';

export type ExportFormatKind = 'csv' | 'json' | 'sql';

/** The export sheet's choices; text fields stay text until validated. */
export type ExportOptions = {
  format: ExportFormatKind;
  delimiter: CsvDelimiterDto;
  header: boolean;
  /** JSON: one object per line (NDJSON) instead of one array. */
  lines: boolean;
  /** SQL: the table the INSERT statements target. */
  table: string;
  /** SQL: rows per INSERT statement, as typed. */
  rowsPerStatement: string;
};

/** Most rows one INSERT may carry (the core clamps to the same limit). */
const MAX_ROWS_PER_STATEMENT = 10_000;

export const formatChoices: readonly {
  value: ExportFormatKind;
  label: string;
}[] = [
  { value: 'csv', label: 'CSV' },
  { value: 'json', label: 'JSON' },
  { value: 'sql', label: 'SQL' },
];

export const delimiterChoices: readonly {
  value: CsvDelimiterDto;
  label: string;
}[] = [
  { value: 'comma', label: 'Comma ( , )' },
  { value: 'semicolon', label: 'Semicolon ( ; )' },
  { value: 'tab', label: 'Tab' },
  { value: 'pipe', label: 'Pipe ( | )' },
];

export const layoutChoices: readonly {
  value: 'array' | 'lines';
  label: string;
}[] = [
  { value: 'array', label: 'Array of objects' },
  { value: 'lines', label: 'One object per line (NDJSON)' },
];

/** CSV with a header row; SQL INSERTs into `table`, 100 rows each. */
export function defaultOptions(table: string): ExportOptions {
  return {
    format: 'csv',
    delimiter: 'comma',
    header: true,
    lines: false,
    table,
    rowsPerStatement: '100',
  };
}

/** Why the SQL options cannot be used, or `null` when they can. */
export function sqlError(options: ExportOptions): string | null {
  if (options.format !== 'sql') return null;
  if (!options.table.trim()) return 'Enter the table name to insert into.';
  const rows = Number(options.rowsPerStatement);
  if (
    !/^\d+$/.test(options.rowsPerStatement.trim()) ||
    rows < 1 ||
    rows > MAX_ROWS_PER_STATEMENT
  )
    return `Rows per INSERT must be a whole number from 1 to ${MAX_ROWS_PER_STATEMENT.toLocaleString()}.`;
  return null;
}

/** The IPC format, or `null` while the options are invalid. */
export function formatDto(options: ExportOptions): ExportFormatDto | null {
  switch (options.format) {
    case 'csv':
      return {
        kind: 'csv',
        delimiter: options.delimiter,
        header: options.header,
      };
    case 'json':
      return { kind: 'json', lines: options.lines };
    case 'sql':
      return sqlError(options)
        ? null
        : {
            kind: 'sql',
            table: options.table.trim(),
            rowsPerStatement: Number(options.rowsPerStatement),
          };
  }
}
