// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { readSettingsFile, writeSettingsFile } from '@openrows/ipc';
import { parseEditorSettings } from '../query/editorSettings';
import { generalSettings, updateGeneral } from './general';
import {
  applySettings,
  collectSettings,
  hydrateSettings,
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
  applySettings({ theme: 'neon', general: { density: 'huge' }, shortcuts: 3 });
  expect(collectSettings()).toMatchObject({
    theme: 'system',
    general: { density: 'regular' },
    shortcuts: {},
  });
});
