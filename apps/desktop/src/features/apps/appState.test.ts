// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { AppCard } from '@openrows/ipc';
import { deriveStatus, preferredProvider, renewable } from './appState';
import type { CheckState } from './appState';

const NOW = 1_000_000;
const LINE = '/app/openrows-mcp --port 32123 --client claude-0a1b2c3d4e5f';

function card(overrides: Partial<AppCard> = {}): AppCard {
  return {
    app: 'claude',
    install: { path: '/bin/claude', version: '2.1.330' },
    link: {
      client: 'claude-0a1b2c3d4e5f',
      token: 'token',
      scope: { provider: 'p', connections: ['c'], expires: NOW + 30 * 86_400 },
      usage: null,
    },
    ...overrides,
  };
}
function done(line: string | null, expected = LINE): CheckState {
  return {
    kind: 'done',
    check: {
      registration: { present: line !== null, line, healthy: true },
      expected,
    },
  };
}

it('reports install state for apps that are not connected', () => {
  expect(deriveStatus(card({ link: null }), undefined, null, NOW).status).toBe(
    'installed',
  );
  expect(
    deriveStatus(card({ link: null, install: null }), undefined, null, NOW)
      .status,
  ).toBe('notInstalled');
});

it('is connected only after the tool confirms the expected entry', () => {
  expect(deriveStatus(card(), { kind: 'pending' }, 32123, NOW).status).toBe(
    'checking',
  );
  expect(deriveStatus(card(), done(LINE), 32123, NOW)).toEqual({
    status: 'connected',
    reasons: [],
  });
});

it('explains every reason a connection needs attention', () => {
  expect(deriveStatus(card(), done(null), 32123, NOW).reasons).toEqual([
    'notRegistered',
  ]);
  expect(
    deriveStatus(card(), done(LINE.replace('32123', '1')), 32123, NOW).reasons,
  ).toEqual(['mismatch']);
  expect(
    deriveStatus(card(), { kind: 'failed', message: 'x' }, null, NOW).reasons,
  ).toEqual(['serverOff', 'unverified']);
  const expired = card();
  if (expired.link?.scope) expired.link.scope.expires = NOW - 1;
  expect(deriveStatus(expired, done(LINE), 32123, NOW).reasons).toEqual([
    'expired',
  ]);
  const revoked = card();
  if (revoked.link) revoked.link.scope = null;
  expect(
    deriveStatus({ ...revoked, install: null }, done(LINE), 32123, NOW).reasons,
  ).toEqual(['missingTool', 'revoked']);
});

it('offers renewal in the last three days', () => {
  expect(renewable(card(), NOW)).toBe(false);
  expect(renewable(card(), NOW + 28 * 86_400)).toBe(true);
});

it('preselects a provider of the app’s own vendor', () => {
  const providers = [
    { id: 'local', kind: 'ollama' },
    { id: 'claude-key', kind: 'anthropic' },
  ];
  expect(preferredProvider('claude', providers, 'local')).toBe('claude-key');
  expect(preferredProvider('gemini', providers, 'local')).toBe('local');
  expect(preferredProvider('codex', [], '')).toBe('');
});
