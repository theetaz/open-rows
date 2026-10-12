// SPDX-License-Identifier: Apache-2.0
import { localSetting } from '../../features/settings/localSetting';

/** `format` keeps each format's own convention (TSV: no header, NULL as text; CSV: header, NULL empty). */
type HeaderSetting = 'format' | 'always' | 'never';
type NullSetting = 'format' | 'text' | 'empty';
/** `minimal` quotes only values that need it; `all` quotes every value. */
type QuoteSetting = 'minimal' | 'all';
/** Field separator of CSV copies; plain copies always use tabs. */
type DelimiterSetting = 'comma' | 'semicolon' | 'tab' | 'pipe';
type QuoteCharSetting = 'double' | 'single';
type NewlineSetting = 'lf' | 'crlf';
/** Decimal separator written in numeric columns. */
type DecimalSetting = 'point' | 'comma';

/** Defaults of plain (TSV), CSV and HTML copies (FR-GRID-13); `settings.json` keeps them under `copy`. */
export type CopySettings = {
  headers: HeaderSetting;
  nulls: NullSetting;
  quote: QuoteSetting;
  delimiter: DelimiterSetting;
  quoteChar: QuoteCharSetting;
  newline: NewlineSetting;
  decimal: DecimalSetting;
};

/** How one copy writes delimited text, resolved from the settings and the format. */
export type DelimitedOptions = {
  headers: boolean;
  /** NULL as the text `NULL`, or as an empty field. */
  nulls: 'text' | 'empty';
  quote: QuoteSetting;
  delimiter: string;
  quoteChar: '"' | "'";
  newline: '\n' | '\r\n';
  decimal: '.' | ',';
};

const defaultCopySettings: CopySettings = {
  headers: 'format',
  nulls: 'format',
  quote: 'minimal',
  delimiter: 'comma',
  quoteChar: 'double',
  newline: 'lf',
  decimal: 'point',
};

/** Key in web storage (and `copy` in settings.json). */
export const COPY_KEY = 'openrows.copy';

const HEADERS: readonly HeaderSetting[] = ['format', 'always', 'never'];
const NULLS: readonly NullSetting[] = ['format', 'text', 'empty'];
const QUOTES: readonly QuoteSetting[] = ['minimal', 'all'];
const DELIMITERS: Readonly<Record<DelimiterSetting, string>> = {
  comma: ',',
  semicolon: ';',
  tab: '\t',
  pipe: '|',
};

function choice<T extends string>(
  value: unknown,
  allowed: readonly T[],
  fallback: T,
): T {
  return allowed.includes(value as T) ? (value as T) : fallback;
}

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseCopySettings(stored: unknown): CopySettings {
  if (typeof stored !== 'object' || stored === null) return defaultCopySettings;
  const value = stored as Record<string, unknown>;
  const fallback = defaultCopySettings;
  return {
    headers: choice(value.headers, HEADERS, fallback.headers),
    nulls: choice(value.nulls, NULLS, fallback.nulls),
    quote: choice(value.quote, QUOTES, fallback.quote),
    delimiter: choice(
      value.delimiter,
      Object.keys(DELIMITERS) as DelimiterSetting[],
      fallback.delimiter,
    ),
    quoteChar: choice(value.quoteChar, ['double', 'single'], 'double'),
    newline: choice(value.newline, ['lf', 'crlf'], 'lf'),
    decimal: choice(value.decimal, ['point', 'comma'], 'point'),
  };
}

/**
 * Why the settings would write ambiguous text, or `null`: a decimal comma
 * inside comma-separated fields cannot be told apart from the delimiter.
 */
export function copyConflict(settings: CopySettings): string | null {
  if (settings.delimiter === 'comma' && settings.decimal === 'comma')
    return 'The CSV delimiter and the decimal separator cannot both be a comma. Choose a semicolon, tab or pipe delimiter, or a point decimal separator.';
  return null;
}

export const copySettings = localSetting(COPY_KEY, parseCopySettings);

/** Change some copy settings; an ambiguous combination is rejected with its reason. */
export function updateCopySettings(
  patch: Partial<CopySettings>,
): string | null {
  const next = { ...copySettings.get(), ...patch };
  const conflict = copyConflict(next);
  if (!conflict) copySettings.set(next);
  return conflict;
}

/** The options of a TSV, CSV or HTML copy; `headers` overrides the setting (Copy with Headers). */
export function delimitedOptions(
  format: 'tsv' | 'csv' | 'html',
  settings: CopySettings,
  headers?: boolean,
): DelimitedOptions {
  const csv = format === 'csv';
  return {
    headers:
      headers ??
      (settings.headers === 'format' ? csv : settings.headers === 'always'),
    nulls:
      settings.nulls === 'format' ? (csv ? 'empty' : 'text') : settings.nulls,
    quote: settings.quote,
    delimiter: csv ? DELIMITERS[settings.delimiter] : '\t',
    quoteChar: settings.quoteChar === 'single' ? "'" : '"',
    newline: settings.newline === 'crlf' ? '\r\n' : '\n',
    decimal: settings.decimal === 'comma' ? ',' : '.',
  };
}
