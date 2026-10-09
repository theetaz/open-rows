// SPDX-License-Identifier: Apache-2.0
import type {
  CsvDelimiterDto,
  CsvQuoteDto,
  LineEndingDto,
  ParquetCompressionDto,
  TextEncodingDto,
} from '@openrows/ipc';
import { localSetting } from '../settings/localSetting';

/** Every export format. */
export type ExportFormatKind =
  'csv' | 'tsv' | 'json' | 'ndjson' | 'sql' | 'markdown' | 'xlsx' | 'parquet';

/** Options of delimited text (CSV and TSV). */
export type Delimited = {
  quote: CsvQuoteDto;
  header: boolean;
  /** Text written for NULL; empty writes an empty field. */
  nullText: string;
  lineEnding: LineEndingDto;
  encoding: TextEncodingDto;
};

/**
 * The last-used export format and the options of every format;
 * `settings.json` keeps them under `export`.
 */
export type ExportSettings = {
  format: ExportFormatKind;
  csv: Delimited & { delimiter: CsvDelimiterDto };
  tsv: Delimited;
  json: { pretty: boolean };
  sql: { rowsPerStatement: number };
  xlsx: { header: boolean };
  parquet: { compression: ParquetCompressionDto };
};

/** Most rows one INSERT may carry (the core clamps to the same limit). */
export const MAX_ROWS_PER_STATEMENT = 10_000;
/** Longest NULL text the core accepts. */
const MAX_NULL_TEXT = 32;

const FORMATS: readonly ExportFormatKind[] = [
  'csv',
  'tsv',
  'json',
  'ndjson',
  'sql',
  'markdown',
  'xlsx',
  'parquet',
];
const DELIMITERS: readonly CsvDelimiterDto[] = [
  'comma',
  'semicolon',
  'tab',
  'pipe',
];
const QUOTES: readonly CsvQuoteDto[] = ['double', 'single'];
const LINE_ENDINGS: readonly LineEndingDto[] = ['crlf', 'lf'];
const ENCODINGS: readonly TextEncodingDto[] = [
  'utf8',
  'utf8Bom',
  'utf16Le',
  'latin1',
];
const COMPRESSIONS: readonly ParquetCompressionDto[] = ['snappy', 'none'];

const delimitedDefaults: Delimited = {
  quote: 'double',
  header: true,
  nullText: '',
  lineEnding: 'crlf',
  encoding: 'utf8',
};

export const defaultExportSettings: ExportSettings = {
  format: 'csv',
  csv: { ...delimitedDefaults, delimiter: 'comma' },
  tsv: delimitedDefaults,
  json: { pretty: false },
  sql: { rowsPerStatement: 100 },
  xlsx: { header: true },
  parquet: { compression: 'snappy' },
};

/** Key in web storage (and `export` in settings.json). */
export const EXPORT_KEY = 'openrows.export';

type Fields = Record<string, unknown>;

const fields = (value: unknown): Fields =>
  typeof value === 'object' && value !== null ? (value as Fields) : {};

function oneOf<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return allowed.find((item) => item === value) ?? fallback;
}

const flag = (value: unknown, fallback: boolean): boolean =>
  typeof value === 'boolean' ? value : fallback;

function delimited(stored: unknown): Delimited {
  const value = fields(stored);
  const nullText =
    typeof value.nullText === 'string' && value.nullText.length <= MAX_NULL_TEXT
      ? value.nullText
      : delimitedDefaults.nullText;
  return {
    quote: oneOf(value.quote, QUOTES, delimitedDefaults.quote),
    header: flag(value.header, delimitedDefaults.header),
    nullText,
    lineEnding: oneOf(
      value.lineEnding,
      LINE_ENDINGS,
      delimitedDefaults.lineEnding,
    ),
    encoding: oneOf(value.encoding, ENCODINGS, delimitedDefaults.encoding),
  };
}

function rowsPerStatement(stored: unknown): number {
  const rows = fields(stored).rowsPerStatement;
  return typeof rows === 'number' &&
    Number.isInteger(rows) &&
    rows >= 1 &&
    rows <= MAX_ROWS_PER_STATEMENT
    ? rows
    : defaultExportSettings.sql.rowsPerStatement;
}

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseExportSettings(stored: unknown): ExportSettings {
  const value = fields(stored);
  return {
    format: oneOf(value.format, FORMATS, defaultExportSettings.format),
    csv: {
      ...delimited(value.csv),
      delimiter: oneOf(fields(value.csv).delimiter, DELIMITERS, 'comma'),
    },
    tsv: delimited(value.tsv),
    json: { pretty: flag(fields(value.json).pretty, false) },
    sql: { rowsPerStatement: rowsPerStatement(value.sql) },
    xlsx: { header: flag(fields(value.xlsx).header, true) },
    parquet: {
      compression: oneOf(
        fields(value.parquet).compression,
        COMPRESSIONS,
        'snappy',
      ),
    },
  };
}

/** The remembered export options, shared live by every export sheet. */
export const exportSettings = localSetting(EXPORT_KEY, parseExportSettings);
