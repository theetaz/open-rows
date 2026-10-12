// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { cancelAssistant, runAssistant } from '@openrows/ipc';
import { inlineGenerator } from './inline';
import { aiSettings, parseAiSettings } from '../settings/ai/aiSettings';
import { profile } from '../../test/ipcStub';
vi.mock('@openrows/ipc', () => ({
  runAssistant: vi.fn(),
  cancelAssistant: vi.fn().mockResolvedValue(undefined),
}));
beforeEach(() => {
  vi.clearAllMocks();
  aiSettings.set(
    parseAiSettings({
      enabled: true,
      defaultProvider: 'fixture',
      defaultModel: 'model',
      providers: [
        {
          id: 'fixture',
          name: 'Fixture',
          kind: 'openAi',
          baseUrl: 'https://api.openai.com/v1/',
          allowInsecureRemote: false,
          secretRef: 'ai/fixture/api-key',
        },
      ],
    }),
  );
});
it('streams selection context through the existing native layer and returns only fenced SQL', async () => {
  vi.mocked(runAssistant).mockImplementation((_, receive) => {
    receive({ type: 'text', delta: '```sql\nSELECT $1;' });
    receive({ type: 'text', delta: '\n```' });
    return Promise.resolve();
  });
  const delta = vi.fn();
  const proposal = await inlineGenerator({
    profile: profile('local'),
    database: 'db',
  })({
    prompt: 'Rewrite',
    sql: 'SELECT 1;',
    signal: new AbortController().signal,
    delta,
  });
  expect(proposal).toBe('SELECT $1;');
  expect(runAssistant).toHaveBeenCalledWith(
    expect.objectContaining({
      inline: true,
      query: 'SELECT 1;',
      provider: 'fixture',
      model: 'model',
    }),
    expect.any(Function),
  );
  expect(delta).toHaveBeenCalledTimes(2);
});
it('aborting calls native cancellation and refuses a late answer', async () => {
  const controller = new AbortController();
  vi.mocked(runAssistant).mockImplementation((request, receive) => {
    controller.abort();
    receive({ type: 'text', delta: '```sql\nDELETE FROM items;\n```' });
    expect(cancelAssistant).toHaveBeenCalledWith(request.id);
    return Promise.resolve();
  });
  const delta = vi.fn();
  await expect(
    inlineGenerator({ profile: profile('local'), database: 'db' })({
      prompt: 'Rewrite',
      sql: '',
      signal: controller.signal,
      delta,
    }),
  ).rejects.toThrow('Cancelled');
  expect(delta).not.toHaveBeenCalled();
});
it('missing model never invokes the provider', async () => {
  aiSettings.set({ ...aiSettings.get(), defaultModel: '' });
  const generate = inlineGenerator({
    profile: profile('local'),
    database: 'db',
  });
  await expect(
    generate({
      prompt: 'Rewrite',
      sql: '',
      signal: new AbortController().signal,
      delta: vi.fn(),
    }),
  ).rejects.toThrow('default provider/model');
  expect(runAssistant).not.toHaveBeenCalled();
});

it('pre-cancelled requests never invoke native generation', async () => {
  const controller = new AbortController();
  controller.abort();
  await expect(
    inlineGenerator({ profile: profile('local'), database: 'db' })({
      prompt: 'Rewrite',
      sql: '',
      signal: controller.signal,
      delta: vi.fn(),
    }),
  ).rejects.toThrow('Cancelled');
  expect(runAssistant).not.toHaveBeenCalled();
});
