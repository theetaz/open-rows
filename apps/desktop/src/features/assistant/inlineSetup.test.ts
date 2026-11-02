// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { CommandError } from '@openrows/ipc';
import { inlineBlock, inlineFailure } from './inlineSetup';
import { streamedSql } from './inline';
import { onSettingsRequest } from '../settings/settingsRequest';
const provider = {
  id: 'fixture',
  name: 'Fixture',
  kind: 'openAi' as const,
  baseUrl: 'https://api.openai.com/v1/',
  allowInsecureRemote: false,
  secretRef: 'ai/fixture/api-key',
};
const ready = {
  enabled: true,
  providers: [provider],
  defaultProvider: 'fixture',
  defaultModel: 'model',
  cliTurnSeconds: 300,
};
it('explains each missing setup step and opens AI settings', () => {
  const open = vi.fn();
  const stop = onSettingsRequest(open);
  expect(inlineBlock({ ...ready, enabled: false })?.reason).toMatch(
    /AI features are off/,
  );
  expect(inlineBlock({ ...ready, providers: [] })?.reason).toMatch(
    /No AI provider is set up/,
  );
  const model = inlineBlock({ ...ready, defaultModel: '' });
  expect(model?.reason).toBe(
    'Choose a default model for Fixture in Settings → AI.',
  );
  model?.action?.run();
  expect(open).toHaveBeenCalledWith('ai');
  expect(inlineBlock(ready)).toBeNull();
  stop();
});
it('maps typed native failures to recovery text', () => {
  const failure = (kind: 'unsupported' | 'auth' | 'timeout' | 'server') =>
    inlineFailure(new CommandError({ kind, message: 'native', code: null }));
  expect(failure('unsupported')).toMatch(/data-sharing policy/);
  expect(failure('auth')).toMatch(/API key/);
  expect(failure('timeout')).toMatch(/did not answer in time/);
  expect(failure('server')).toBe('native');
  expect(inlineFailure('opaque')).toBe('Inline request failed.');
});
it('shows only the SQL body of a partially streamed fence', () => {
  expect(streamedSql('Here:\n```sql\nSELECT 1')).toBe('SELECT 1');
  expect(streamedSql('```sql\nSELECT 1;\n```\nDone')).toBe('SELECT 1;\n');
  expect(streamedSql('thinking')).toBe('thinking');
});
