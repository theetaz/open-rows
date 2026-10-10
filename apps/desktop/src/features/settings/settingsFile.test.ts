// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import {
  onSettingsFileChanged,
  readSettingsFile,
  writeSettingsFile,
} from '@openrows/ipc';
import { COMMANDS } from '../commands';
import { parseEditorSettings } from '../query/editorSettings';
import { generalSettings, updateGeneral } from './general';
import { onSettingsReload } from './localSetting';
import {
  applySettings,
  collectSettings,
  hydrateSettings,
  reloadSettingsFile,
} from './settingsFile';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

/** The JSON text of the last settings.json write. */
function written(): Record<string, unknown> {
  const text = vi.mocked(writeSettingsFile).mock.lastCall?.[0];
  if (!text) throw new Error('settings.json was not written');
  return JSON.parse(text) as Record<string, unknown>;
}

it('migrates existing device settings into a new settings.json', async () => {
  vi.mocked(writeSettingsFile).mockClear();
  localStorage.setItem('openrows.theme', 'dark');
  localStorage.setItem(
    'openrows.appearance.v1',
    JSON.stringify({ uiSize: 15, contrast: 999 }),
  );
  localStorage.setItem(
    'openrows.editor',
    JSON.stringify({ wrap: true, rowLimit: 7 }),
  );
  localStorage.setItem('openrows.shortcuts', JSON.stringify({ save: 'Cmd+J' }));
  vi.mocked(readSettingsFile).mockResolvedValueOnce(null);
  await hydrateSettings();
  const file = written();
  expect(file).toMatchObject({
    openrows: 1,
    theme: 'dark',
    appearance: { uiSize: 15, contrast: 150 },
    editor: { wrap: true, rowLimit: null },
    shortcuts: { save: 'Cmd+J' },
    general: { restoreSession: true, density: 'regular' },
    results: { memoryBudgetMb: 512 },
    history: { maxEntries: 100000, maxAgeDays: 90, keepForever: false },
  });
  expect(vi.mocked(writeSettingsFile).mock.lastCall?.[0]).toContain(
    '\n  "theme": "dark",\n',
  );
});

it('loads settings.json into the stores and rewrites it after a change', async () => {
  vi.useFakeTimers();
  vi.mocked(writeSettingsFile).mockClear();
  vi.mocked(readSettingsFile).mockResolvedValueOnce(
    JSON.stringify({
      openrows: 1,
      theme: 'light',
      general: { restoreSession: false, density: 'compact' },
      editor: { keyBinding: 'vim' },
    }),
  );
  await hydrateSettings();
  expect(writeSettingsFile).not.toHaveBeenCalled();
  expect(localStorage.getItem('openrows.theme')).toBe('light');
  expect(generalSettings.get()).toMatchObject({
    restoreSession: false,
    density: 'compact',
  });
  expect(
    parseEditorSettings(localStorage.getItem('openrows.editor')).keyBinding,
  ).toBe('vim');
  updateGeneral({ reduceMotion: true });
  vi.advanceTimersByTime(400);
  expect(written()).toMatchObject({ general: { reduceMotion: true } });
  vi.useRealTimers();
});

it('keeps invalid values out of the stores and the file', () => {
  applySettings({
    theme: 'neon',
    general: { density: 'huge', keepaliveSeconds: 7 },
    shortcuts: 3,
  });
  expect(collectSettings()).toMatchObject({
    theme: 'system',
    general: { density: 'regular', keepaliveSeconds: 30 },
    shortcuts: {},
  });
});

it('keeps a valid result memory budget and replaces an invalid one', () => {
  applySettings({ results: { memoryBudgetMb: 2048 } });
  expect(collectSettings()).toMatchObject({
    results: { memoryBudgetMb: 2048 },
  });
  for (const memoryBudgetMb of [0, 63, 65_537, 1.5, '1024']) {
    applySettings({ results: { memoryBudgetMb } });
    expect(collectSettings()).toMatchObject({
      results: { memoryBudgetMb: 512 },
    });
  }
});

it('reloads an externally edited file into every store', async () => {
  let changed: ((change: Record<string, never>) => void) | undefined;
  vi.mocked(onSettingsFileChanged).mockImplementationOnce((listener) => {
    changed = listener;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(readSettingsFile).mockResolvedValueOnce(null);
  await hydrateSettings();
  const seen = vi.fn();
  const stop = onSettingsReload(seen);
  vi.mocked(readSettingsFile).mockResolvedValueOnce(
    JSON.stringify({ openrows: 1, general: { density: 'comfortable' } }),
  );
  changed?.({});
  await vi.waitFor(() => {
    expect(seen).toHaveBeenCalledOnce();
  });
  expect(generalSettings.get().density).toBe('comfortable');
  vi.mocked(readSettingsFile).mockResolvedValueOnce('{ broken');
  await reloadSettingsFile();
  vi.mocked(readSettingsFile).mockResolvedValueOnce('[1]');
  await reloadSettingsFile();
  expect(seen).toHaveBeenCalledOnce();
  stop();
});

it('describes every section and field of settings.json in its JSON Schema', () => {
  const schema = JSON.parse(
    readFileSync('apps/desktop/src-tauri/settings.schema.json', 'utf8'),
  ) as {
    properties: Record<
      string,
      {
        properties?: Record<string, unknown>;
        propertyNames?: { enum: string[] };
      }
    >;
  };
  const file = collectSettings();
  expect(Object.keys(schema.properties).sort()).toEqual(
    ['$schema', ...Object.keys(file)].sort(),
  );
  for (const [name, value] of Object.entries(file)) {
    if (name === 'shortcuts' || typeof value !== 'object' || value === null)
      continue;
    expect(
      Object.keys(schema.properties[name]?.properties ?? {}).sort(),
      name,
    ).toEqual(Object.keys(value).sort());
  }
  expect(schema.properties.shortcuts?.propertyNames?.enum).toEqual(
    COMMANDS.map(({ id }) => id),
  );
});
