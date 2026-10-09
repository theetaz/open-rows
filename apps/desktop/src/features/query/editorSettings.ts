// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import type { EditorPreferences, FormatOptions } from '@openrows/editor';
import type { ParamStyleDto } from '@openrows/ipc';
import type { RunScope } from './request';

/** Key handling of the SQL editor. */
export type KeyBinding = 'default' | 'vim';

/** Placeholders treated as query parameters; `off` runs SQL as written. */
export type QueryParams = 'off' | ParamStyleDto;

/** Multi-statement runs: one result tab per statement, or only the last result. */
export type ResultView = 'tabs' | 'last';

/**
 * SQL editor settings from the bottom-bar menus, shared by every query tab
 * and persisted on this device (like the appearance settings).
 */
export type EditorSettings = EditorPreferences &
  Omit<FormatOptions, 'dialect'> & {
    keyBinding: KeyBinding;
    /** Rows kept per result set when running; `null` is no limit. */
    rowLimit: number | null;
    /** Keep query tab text (and bound files) saved as you type. */
    autoSave: boolean;
    queryParams: QueryParams;
    resultView: ResultView;
    /** Stop a multi-statement run at the first error. */
    returnOnError: boolean;
    /** What the Run button and ⌘↵ run (a selection always runs itself). */
    runDefault: RunScope;
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
  indentStyle: 'standard',
  autoSave: true,
  queryParams: 'off',
  resultView: 'tabs',
  returnOnError: true,
  runDefault: 'current',
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
  indentStyle: oneOf<FormatOptions['indentStyle']>(
    'standard',
    'tabularLeft',
    'tabularRight',
  ),
  autoSave: isBoolean,
  queryParams: oneOf<QueryParams>('off', 'named', 'numbered', 'anonymous'),
  resultView: oneOf<ResultView>('tabs', 'last'),
  returnOnError: isBoolean,
  runDefault: oneOf<RunScope>('current', 'all'),
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

const listeners = new Set<() => void>();
/** The last parsed text and its settings, so snapshots stay referentially stable. */
let cached: { text: string | null; settings: EditorSettings } = {
  text: null,
  settings: defaultEditorSettings,
};
/** Settings of this session when storage is unavailable. */
let unstored: EditorSettings | null = null;

function snapshot(): EditorSettings {
  if (unstored) return unstored;
  let text: string | null;
  try {
    text = localStorage.getItem(STORAGE_KEY);
  } catch {
    return defaultEditorSettings;
  }
  if (text !== cached.text)
    cached = { text, settings: parseEditorSettings(text) };
  return cached.settings;
}

function save(settings: EditorSettings): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    unstored = null;
  } catch {
    unstored = settings;
  }
  listeners.forEach((listener) => {
    listener();
  });
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Merge a partial change into the persisted settings. */
function updateEditorSettings(change: Partial<EditorSettings>): void {
  save({ ...snapshot(), ...change });
}

/** Persisted editor settings, shared live by every caller, and an updater. */
export function useEditorSettings(): [
  EditorSettings,
  (change: Partial<EditorSettings>) => void,
] {
  const settings = useSyncExternalStore(subscribe, snapshot);
  return [settings, updateEditorSettings];
}
