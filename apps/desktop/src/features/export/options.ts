// SPDX-License-Identifier: Apache-2.0
import type {
  CsvDelimiterDto,
  CsvQuoteDto,
  ExportFormatDto,
  LineEndingDto,
  ParquetCompressionDto,
  TextEncodingDto,
} from '@openrows/ipc';
import { MAX_ROWS_PER_STATEMENT } from './exportSettings';
import type { ExportFormatKind, ExportSettings } from './exportSettings';
import { t } from './lazyMessages';

/** The export sheet's choices; text fields stay text until validated. */
export type ExportOptions = ExportSettings & {
  /** SQL: the table the INSERT statements target. */
  table: string;
  /** SQL: rows per INSERT statement, as typed. */
  rowsText: string;
};

type Choice<Value extends string> = { value: Value; label: string };

export const formatChoices: readonly Choice<ExportFormatKind>[] = [
  { value: 'csv', label: t('csv') },
  { value: 'tsv', label: t('tsv') },
  { value: 'json', label: t('json') },
  { value: 'ndjson', label: t('ndjson') },
  { value: 'sql', label: t('sqlInsert') },
  { value: 'markdown', label: t('markdown') },
  { value: 'xlsx', label: t('excelXlsx') },
  { value: 'parquet', label: t('parquet') },
];

export const delimiterChoices: readonly Choice<CsvDelimiterDto>[] = [
  { value: 'comma', label: t('comma') },
  { value: 'semicolon', label: t('semicolon') },
  { value: 'tab', label: t('tab') },
  { value: 'pipe', label: t('pipe') },
];

export const quoteChoices: readonly Choice<CsvQuoteDto>[] = [
  { value: 'double', label: t('double') },
  { value: 'single', label: t('single') },
];

export const lineEndingChoices: readonly Choice<LineEndingDto>[] = [
  { value: 'crlf', label: t('crlfWindowsRfc') },
  { value: 'lf', label: t('lfMacosLinux') },
];

export const encodingChoices: readonly Choice<TextEncodingDto>[] = [
  { value: 'utf8', label: t('utf8') },
  { value: 'utf8Bom', label: t('utf8Bom') },
  { value: 'utf16Le', label: t('utf16Le') },
  { value: 'latin1', label: t('latin1Iso') },
];

export const layoutChoices: readonly Choice<'compact' | 'pretty'>[] = [
  { value: 'compact', label: t('compactOneObject') },
  { value: 'pretty', label: t('prettyIndented') },
];

export const compressionChoices: readonly Choice<ParquetCompressionDto>[] = [
  { value: 'snappy', label: t('snappy') },
  { value: 'none', label: t('none') },
];

/** The remembered options with the SQL target `table`. */
export function sheetOptions(
  settings: ExportSettings,
  table: string,
): ExportOptions {
  return {
    ...settings,
    table,
    rowsText: String(settings.sql.rowsPerStatement),
  };
}

/** What to remember of the sheet's options. */
export function rememberedOptions(options: ExportOptions): ExportSettings {
  const settings: ExportSettings = {
    format: options.format,
    csv: options.csv,
    tsv: options.tsv,
    json: options.json,
    sql: options.sql,
    xlsx: options.xlsx,
    parquet: options.parquet,
  };
  return rowsError(options.rowsText)
    ? settings
    : { ...settings, sql: { rowsPerStatement: Number(options.rowsText) } };
}

/** Why typed rows per INSERT are invalid, or `null`. */
function rowsError(text: string): string | null {
  const rows = Number(text);
  return /^\d+$/.test(text.trim()) &&
    rows >= 1 &&
    rows <= MAX_ROWS_PER_STATEMENT
    ? null
    : t('rowsPerInsertMust', {
        maxRowsPerStatement: MAX_ROWS_PER_STATEMENT.toLocaleString(),
      });
}

/** Why the SQL options cannot be used, or `null` when they can. */
export function sqlError(options: ExportOptions): string | null {
  if (options.format !== 'sql') return null;
  if (!options.table.trim()) return t('enterTableName');
  return rowsError(options.rowsText);
}

/** Why the NULL text cannot be used with these options, or `null`. */
export function nullTextError(options: ExportOptions): string | null {
  const text =
    options.format === 'csv'
      ? options.csv
      : options.format === 'tsv'
        ? options.tsv
        : null;
  if (!text) return null;
  const delimiter =
    options.format === 'tsv'
      ? '\t'
      : { comma: ',', semicolon: ';', tab: '\t', pipe: '|' }[
          options.csv.delimiter
        ];
  const quote = text.quote === 'double' ? '"' : "'";
  return [delimiter, quote, '\r', '\n'].some((part) =>
    text.nullText.includes(part),
  ) || text.nullText.length > 32
    ? t('nullTextCannot')
    : null;
}

/** The IPC format, or `null` while the options are invalid. */
export function formatDto(options: ExportOptions): ExportFormatDto | null {
  if (sqlError(options) || nullTextError(options)) return null;
  const { csv, tsv } = options;
  switch (options.format) {
    case 'csv': {
      const { delimiter, ...rest } = csv;
      return { kind: 'csv', delimiter, options: rest };
    }
    case 'tsv':
      return { kind: 'tsv', options: tsv };
    case 'json':
      return { kind: 'json', pretty: options.json.pretty };
    case 'sql':
      return {
        kind: 'sql',
        table: options.table.trim(),
        rowsPerStatement: Number(options.rowsText),
      };
    case 'xlsx':
      return { kind: 'xlsx', header: options.xlsx.header };
    case 'parquet':
      return { kind: 'parquet', compression: options.parquet.compression };
    case 'ndjson':
    case 'markdown':
      return { kind: options.format };
  }
}
