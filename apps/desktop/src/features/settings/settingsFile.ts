// SPDX-License-Identifier: Apache-2.0
import { readSettingsFile, writeSettingsFile } from '@openrows/ipc';
import { parseAppearance } from '@openrows/ui';
import { parseEditorSettings } from '../query/editorSettings';
import { parseOverrides, SHORTCUTS_KEY } from '../commands/bindings';
import { GENERAL_KEY, parseGeneral } from './general';
import { onSettingsChanged } from './localSetting';

/**
 * `settings.json` (docs/15 §4) mirrors these web-storage keys, which stay
 * the synchronous cache every store reads. The file wins on launch; before
 * it exists, the keys are migrated into it.
 */
const THEME_KEY = 'openrows.theme';
const APPEARANCE_KEY = 'openrows.appearance.v1';
const EDITOR_KEY = 'openrows.editor';

/** Debounce of file writes after a change. */
const WRITE_DELAY_MS = 300;

type Section = {
  key: string;
  parse: (stored: unknown) => unknown;
};

/** File sections and their stores; every value is validated on the way in and out. */
const SECTIONS: Readonly<Record<string, Section>> = {
  general: { key: GENERAL_KEY, parse: parseGeneral },
  appearance: { key: APPEARANCE_KEY, parse: parseAppearance },
  editor: {
    key: EDITOR_KEY,
    parse: (stored) => parseEditorSettings(JSON.stringify(stored ?? null)),
  },
  shortcuts: { key: SHORTCUTS_KEY, parse: parseOverrides },
};

const themeOf = (value: unknown): string =>
  value === 'light' || value === 'dark' ? value : 'system';

function stored(key: string): unknown {
  try {
    const text = localStorage.getItem(key);
    return text ? (JSON.parse(text) as unknown) : null;
  } catch {
    return null;
  }
}

/** Every setting of this device as the `settings.json` object. */
export function collectSettings(): Record<string, unknown> {
  let theme: string | null = null;
  try {
    theme = localStorage.getItem(THEME_KEY);
  } catch {
    theme = null;
  }
  const sections = Object.entries(SECTIONS).map(
    ([name, section]): [string, unknown] => [
      name,
      section.parse(stored(section.key)),
    ],
  );
  return {
    openrows: 1,
    theme: themeOf(theme),
    ...Object.fromEntries(sections),
  };
}

/** Load a `settings.json` object into the stores' web-storage keys. */
export function applySettings(file: Record<string, unknown>): void {
  try {
    localStorage.setItem(THEME_KEY, themeOf(file.theme));
    for (const [name, section] of Object.entries(SECTIONS)) {
      if (name in file)
        localStorage.setItem(
          section.key,
          JSON.stringify(section.parse(file[name])),
        );
    }
  } catch {
    /* Without web storage the defaults apply for this session. */
  }
}

/** The file as text: two-space JSON, human-readable and diff-able. */
export function settingsText(): string {
  return `${JSON.stringify(collectSettings(), null, 2)}\n`;
}

function writer(): () => void {
  let timer: number | undefined;
  const flush = (): void => {
    if (timer === undefined) return;
    window.clearTimeout(timer);
    timer = undefined;
    writeSettingsFile(settingsText()).catch(() => undefined);
  };
  window.addEventListener('pagehide', flush);
  return () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(flush, WRITE_DELAY_MS);
  };
}

/**
 * Before the first render: load `settings.json` into the stores, or create
 * it from this device's existing settings (migration). An unreadable file is
 * left untouched and settings stay device-local for the session.
 */
export async function hydrateSettings(): Promise<void> {
  let text: string | null;
  try {
    text = await readSettingsFile();
  } catch {
    return;
  }
  if (text) applySettings(JSON.parse(text) as Record<string, unknown>);
  else await writeSettingsFile(settingsText()).catch(() => undefined);
  onSettingsChanged(writer());
}
