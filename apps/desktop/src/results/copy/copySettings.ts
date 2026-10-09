// SPDX-License-Identifier: Apache-2.0
import { localSetting } from '../../features/settings/localSetting';

/** `format` keeps each format's own convention (TSV: no header, NULL as text; CSV: header, NULL empty). */
type HeaderSetting = 'format' | 'always' | 'never';
type NullSetting = 'format' | 'text' | 'empty';
/** `minimal` quotes only values that need it; `all` quotes every value. */
type QuoteSetting = 'minimal' | 'all';

/** Defaults of plain (TSV) and CSV copies (FR-GRID-13); `settings.json` keeps them under `copy`. */
export type CopySettings = {
  headers: HeaderSetting;
  nulls: NullSetting;
  quote: QuoteSetting;
};

/** How one copy writes delimited text, resolved from the settings and the format. */
export type DelimitedOptions = {
  headers: boolean;
  /** NULL as the text `NULL`, or as an empty field. */
  nulls: 'text' | 'empty';
  quote: QuoteSetting;
};

const defaultCopySettings: CopySettings = {
  headers: 'format',
  nulls: 'format',
  quote: 'minimal',
};

/** Key in web storage (and `copy` in settings.json). */
export const COPY_KEY = 'openrows.copy';

const HEADERS: readonly HeaderSetting[] = ['format', 'always', 'never'];
const NULLS: readonly NullSetting[] = ['format', 'text', 'empty'];
const QUOTES: readonly QuoteSetting[] = ['minimal', 'all'];

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
  return {
    headers: choice(value.headers, HEADERS, defaultCopySettings.headers),
    nulls: choice(value.nulls, NULLS, defaultCopySettings.nulls),
    quote: choice(value.quote, QUOTES, defaultCopySettings.quote),
  };
}

export const copySettings = localSetting(COPY_KEY, parseCopySettings);

/** Change some copy settings. */
export function updateCopySettings(patch: Partial<CopySettings>): void {
  copySettings.set({ ...copySettings.get(), ...patch });
}

/** The options of a TSV or CSV copy; `headers` overrides the setting (Copy with Headers). */
export function delimitedOptions(
  format: 'tsv' | 'csv',
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
  };
}
