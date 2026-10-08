// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { EditorPreferences, FormatOptions } from '@openrows/editor';

/** Key handling of the SQL editor. */
export type KeyBinding = 'default' | 'vim';

/**
 * SQL editor settings from the bottom-bar menus, shared by every query tab
 * and persisted on this device (like the appearance settings).
 */
export type EditorSettings = EditorPreferences &
  Omit<FormatOptions, 'dialect'> & {
    keyBinding: KeyBinding;
    /** Rows kept per result set when running; `null` is no limit. */
    rowLimit: number | null;
  };

/** Settings before the user changed anything. */
export const defaultEditorSettings: EditorSettings = {
  fontSize: null,
  invisibles: false,
  wrap: false,
  highlightQuery: false,
  autocomplete: true,
  upperKeywords: true,
  keyBinding: 'default',
  rowLimit: null,
  indentWidth: 2,
  indentWith: 'spaces',
  keywordCase: 'upper',
};

/** Editor font sizes offered besides the appearance default. */
export const FONT_SIZES = [11, 12, 13, 14, 15, 16, 18, 20, 24] as const;
/** Row limits offered besides "No limit". */
export const ROW_LIMITS = [
  100, 500, 1_000, 5_000, 10_000, 50_000, 100_000, 500_000,
] as const;

const STORAGE_KEY = 'openrows.editor';

type Field<K extends keyof EditorSettings> = (
  value: unknown,
) => value is EditorSettings[K];

const isBoolean = (value: unknown): value is boolean =>
  typeof value === 'boolean';
const oneOf =
  <T>(...allowed: readonly T[]) =>
  (value: unknown): value is T =>
    allowed.includes(value as T);

/** Validators of every persisted field; unknown or invalid values are dropped. */
const FIELDS: { [K in keyof EditorSettings]: Field<K> } = {
  fontSize: oneOf<number | null>(null, ...FONT_SIZES),
  invisibles: isBoolean,
  wrap: isBoolean,
  highlightQuery: isBoolean,
  autocomplete: isBoolean,
  upperKeywords: isBoolean,
  keyBinding: oneOf<KeyBinding>('default', 'vim'),
  rowLimit: oneOf<number | null>(null, ...ROW_LIMITS),
  indentWidth: oneOf<2 | 4>(2, 4),
  indentWith: oneOf<'spaces' | 'tabs'>('spaces', 'tabs'),
  keywordCase: oneOf<FormatOptions['keywordCase']>(
    'preserve',
    'upper',
    'lower',
  ),
};

/** Parse persisted settings, keeping defaults for anything missing or invalid. */
export function parseEditorSettings(text: string | null): EditorSettings {
  let stored: unknown;
  try {
    stored = text ? JSON.parse(text) : null;
  } catch {
    stored = null;
  }
  if (typeof stored !== 'object' || stored === null)
    return defaultEditorSettings;
  const record = stored as Record<string, unknown>;
  const valid = Object.entries(FIELDS).filter(([key, check]) =>
    check(record[key]),
  );
  return {
    ...defaultEditorSettings,
    ...Object.fromEntries(valid.map(([key]) => [key, record[key]])),
  };
}

function read(): EditorSettings {
  try {
    return parseEditorSettings(localStorage.getItem(STORAGE_KEY));
  } catch {
    return defaultEditorSettings;
  }
}

function save(settings: EditorSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    /* Settings still apply for this session without storage. */
  }
}

/** Persisted editor settings and an updater merging a partial change. */
export function useEditorSettings(): [
  EditorSettings,
  (change: Partial<EditorSettings>) => void,
] {
  const [settings, setSettings] = useState(read);
  const update = (change: Partial<EditorSettings>): void => {
    setSettings((current) => {
      const next = { ...current, ...change };
      save(next);
      return next;
    });
  };
  return [settings, update];
}
