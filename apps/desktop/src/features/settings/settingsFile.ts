// SPDX-License-Identifier: Apache-2.0
import {
  onSettingsFileChanged,
  readSettingsFile,
  writeSettingsFile,
} from '@openrows/ipc';
import { parseAppearance } from '@openrows/ui';
import { parseEditorSettings } from '../query/editorSettings';
import { EXECUTION_KEY, parseExecution } from '../query/executionSettings';
import { parseOverrides, SHORTCUTS_KEY } from '../commands/bindings';
import { DIAGNOSTICS_KEY, parseDiagnostics } from './diagnostics';
import { GENERAL_KEY, parseGeneral } from './general';
import { parseResults, RESULTS_KEY } from './results';
import { parseSnippets, SNIPPETS_KEY } from '../snippets/snippetStore';
import { EXPORT_KEY, parseExportSettings } from '../export/exportSettings';
import { COPY_KEY, parseCopySettings } from '../../results/copy/copySettings';
import { onSettingsChanged, reloadSettings } from './localSetting';
import { parseTools, TOOLS_KEY } from './toolSettings';

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
  execution: { key: EXECUTION_KEY, parse: parseExecution },
  shortcuts: { key: SHORTCUTS_KEY, parse: parseOverrides },
  diagnostics: { key: DIAGNOSTICS_KEY, parse: parseDiagnostics },
  results: { key: RESULTS_KEY, parse: parseResults },
  snippets: { key: SNIPPETS_KEY, parse: parseSnippets },
  export: { key: EXPORT_KEY, parse: parseExportSettings },
  copy: { key: COPY_KEY, parse: parseCopySettings },
  tools: { key: TOOLS_KEY, parse: parseTools },
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
  await onSettingsFileChanged(() => {
    void reloadSettingsFile();
  }).catch(() => undefined);
}

/**
 * After an edit outside the app: load the file into the stores and refresh
 * every reader. A file that no longer parses or was removed is ignored, so
 * the settings in use stay until it is fixed.
 */
export async function reloadSettingsFile(): Promise<void> {
  let file: unknown;
  try {
    const text = await readSettingsFile();
    file = text ? (JSON.parse(text) as unknown) : null;
  } catch {
    return;
  }
  if (typeof file !== 'object' || file === null || Array.isArray(file)) return;
  applySettings(file as Record<string, unknown>);
  reloadSettings();
}
