// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { parseAiSettings } from './aiSettings';
import {
  applySettings,
  collectSettings,
  exportSettings,
  importSettings,
  resetSettings,
} from '../settingsFile';
import { parsePortableSettings } from '../settingsValidation';
vi.mock('@openrows/ipc', async () =>
  (await import('../../../test/ipcStub')).ipcStub(),
);
it('defaults AI off and mirrors only reference fields into settings.json', () => {
  expect(parseAiSettings(null)).toEqual({
    enabled: false,
    providers: [],
    defaultProvider: '',
    defaultModel: '',
  });
  const provider = {
    id: 'fixture',
    name: 'Fixture',
    kind: 'ollama',
    baseUrl: 'http://127.0.0.1:11434/',
    allowInsecureRemote: false,
    secretRef: '',
    apiKey: 'test-only',
  };
  applySettings({
    ai: { enabled: false, providers: [provider], apiKey: 'test-only' },
  });
  const settings = collectSettings();
  expect(JSON.stringify(settings.ai)).not.toContain('test-only');
  expect(settings.ai).toMatchObject({
    enabled: false,
    providers: [{ secretRef: '' }],
  });
  expect(() =>
    parsePortableSettings(
      JSON.stringify({ openrows: 1, ai: { providers: [provider] } }),
    ),
  ).toThrow(/unknown key/);
});
it('rejects foreign keychain references and clears missing default selection', () => {
  const provider = {
    id: 'fixture',
    name: 'Fixture',
    kind: 'openAi',
    baseUrl: 'https://api.openai.com/v1/',
    allowInsecureRemote: false,
    secretRef: 'conn/other/password',
  };
  expect(
    parseAiSettings({
      providers: [provider],
      defaultProvider: 'fixture',
      defaultModel: 'model',
    }),
  ).toMatchObject({ providers: [], defaultProvider: '', defaultModel: '' });
});

it('keeps provider references on reset and excludes device credentials from portable settings', async () => {
  const provider = {
    id: 'fixture',
    name: 'Fixture',
    kind: 'openAi',
    baseUrl: 'https://api.openai.com/v1/',
    allowInsecureRemote: false,
    secretRef: 'ai/fixture/api-key',
  };
  applySettings({
    ai: {
      enabled: true,
      providers: [provider],
      defaultProvider: 'fixture',
      defaultModel: 'model',
    },
  });
  resetSettings('ai');
  expect(collectSettings().ai).toMatchObject({
    enabled: false,
    providers: [provider],
    defaultProvider: '',
    defaultModel: '',
  });
  expect(JSON.parse(exportSettings())).not.toHaveProperty('ai');
  await expect(
    importSettings(JSON.stringify({ openrows: 1, ai: { enabled: true } })),
  ).rejects.toThrow('excluded from portable settings');
});
