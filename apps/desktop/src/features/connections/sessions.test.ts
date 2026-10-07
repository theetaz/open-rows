// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { applySessionEvent, statusLabel } from './sessions';
import type { Statuses } from './sessions';
import { server } from '../../test/ipcStub';

const statuses: Statuses = {
  a: { state: 'connected', sessionId: 's-a', server: server('17.2') },
  b: { state: 'connecting' },
};

it('marks the owning profile failed or disconnected from core events', () => {
  const failed = applySessionEvent(statuses, {
    sessionId: 's-a',
    state: 'failed',
    message: 'network error: broken pipe',
  });
  expect(failed.a).toEqual({
    state: 'failed',
    message: 'network error: broken pipe',
  });
  const closed = applySessionEvent(statuses, {
    sessionId: 's-a',
    state: 'closed',
    message: null,
  });
  expect(closed.a).toEqual({ state: 'disconnected' });
});

it('ignores unknown sessions and non-terminal states', () => {
  const unknown = {
    sessionId: 'other',
    state: 'failed',
    message: null,
  } as const;
  expect(applySessionEvent(statuses, unknown)).toBe(statuses);
  const busy = { sessionId: 's-a', state: 'busy', message: null } as const;
  expect(applySessionEvent(statuses, busy)).toBe(statuses);
});

it('labels every state', () => {
  expect(statusLabel({ state: 'connecting' })).toBe('Connecting…');
  expect(
    statusLabel({ state: 'connected', sessionId: 's', server: server('') }),
  ).toBe('Connected');
  expect(
    statusLabel({
      state: 'connected',
      sessionId: 's',
      server: { ...server('16.4'), flavour: 'Neon' },
    }),
  ).toBe('Connected · PostgreSQL 16.4 · Neon');
  expect(statusLabel({ state: 'failed', message: 'x' })).toBe(
    'Connection failed',
  );
  expect(statusLabel({ state: 'disconnected' })).toBe('Not connected');
});
