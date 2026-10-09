// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  applySessionEvent,
  removeDatabase,
  sessionOf,
  setDatabase,
  statusLabel,
  statusTone,
} from './sessions';
import type { SessionStatus, Statuses } from './sessions';
import { neighbour } from './useDatabases';
import { server } from '../../test/ipcStub';

const connected = (
  databases: Extract<SessionStatus, { state: 'connected' }>['databases'],
): SessionStatus => ({
  state: 'connected',
  insecure: [],
  server: server('17.2'),
  databases,
  generation: 1,
});

const statuses: Statuses = {
  a: connected([
    { name: 'orders', state: 'open', sessionId: 's-orders' },
    { name: 'reports', state: 'open', sessionId: 's-reports' },
  ]),
  b: { state: 'connecting' },
};

it('marks the owning database failed or closed from core events', () => {
  const failed = applySessionEvent(statuses, {
    sessionId: 's-reports',
    state: 'failed',
    message: 'network error: broken pipe',
  });
  expect(failed.a).toMatchObject({
    state: 'connected',
    insecure: [],
    databases: [
      { name: 'orders', state: 'open' },
      {
        name: 'reports',
        state: 'failed',
        message: 'network error: broken pipe',
      },
    ],
  });
  const closed = applySessionEvent(statuses, {
    sessionId: 's-orders',
    state: 'closed',
    message: null,
  });
  expect(closed.a).toMatchObject({
    databases: [{ name: 'reports', state: 'open' }],
  });
});

it('ignores unknown sessions and non-terminal states', () => {
  const unknown = {
    sessionId: 'other',
    state: 'failed',
    message: null,
  } as const;
  expect(applySessionEvent(statuses, unknown)).toBe(statuses);
  const busy = { sessionId: 's-orders', state: 'busy', message: null } as const;
  expect(applySessionEvent(statuses, busy)).toBe(statuses);
});

it('keeps each opened database in place and only on connected profiles', () => {
  const opening = setDatabase(statuses, 'a', {
    name: 'archive',
    state: 'opening',
  });
  const opened = setDatabase(opening, 'a', {
    name: 'archive',
    state: 'open',
    sessionId: 's-archive',
  });
  const status = opened.a ?? { state: 'disconnected' };
  expect(sessionOf(status, 'archive')).toBe('s-archive');
  expect(sessionOf(status, 'orders')).toBe('s-orders');
  expect(sessionOf(status, 'missing')).toBeNull();
  expect(sessionOf(opening.a ?? status, 'archive')).toBeNull();
  expect(setDatabase(statuses, 'b', { name: 'x', state: 'opening' })).toBe(
    statuses,
  );
  const removed = removeDatabase(opened, 'a', 'orders');
  expect(sessionOf(removed.a ?? status, 'orders')).toBeNull();
});

it('makes a neighbour active when the active database closes', () => {
  const databases = [
    { name: 'a', state: 'opening' },
    { name: 'b', state: 'opening' },
    { name: 'c', state: 'opening' },
  ] as const;
  expect(neighbour(databases, 'b')).toBe('a');
  expect(neighbour(databases, 'a')).toBe('b');
  expect(neighbour([databases[0]], 'a')).toBeUndefined();
});

it('maps connection states to the rail status dot', () => {
  expect(statusTone(connected([]))).toBe('connected');
  expect(statusTone({ state: 'connecting' })).toBe('connecting');
  expect(statusTone({ state: 'failed', message: 'x' })).toBe('failed');
  expect(statusTone({ state: 'disconnected' })).toBe('none');
});

it('labels every state', () => {
  expect(statusLabel({ state: 'connecting' })).toBe('Connecting…');
  expect(
    statusLabel({ ...connected([]), server: server('') } as SessionStatus),
  ).toBe('Connected');
  expect(
    statusLabel({
      ...connected([]),
      server: { ...server('16.4'), flavour: 'Neon' },
    } as SessionStatus),
  ).toBe('Connected · PostgreSQL 16.4 · Neon');
  expect(statusLabel({ state: 'failed', message: 'x' })).toBe(
    'Connection failed',
  );
  expect(statusLabel({ state: 'disconnected' })).toBe('Not connected');
});

it('tracks a lost connection through reconnecting back to open (M1-08)', () => {
  const event = (state: 'reconnecting' | 'idle' | 'failed') =>
    ({ sessionId: 's-orders', state, message: 'connection reset' }) as const;
  const reconnecting = applySessionEvent(statuses, event('reconnecting'));
  const status = reconnecting.a ?? { state: 'disconnected' };
  expect(status).toMatchObject({
    databases: [
      {
        name: 'orders',
        state: 'reconnecting',
        sessionId: 's-orders',
        message: 'connection reset',
      },
      { name: 'reports', state: 'open' },
    ],
  });
  expect(sessionOf(status, 'orders')).toBe('s-orders');
  expect(statusTone(status)).toBe('reconnecting');
  expect(statusLabel(status)).toBe('Reconnecting…');
  const back = applySessionEvent(reconnecting, event('idle'));
  expect(back.a).toEqual(statuses.a);
  const lost = applySessionEvent(reconnecting, event('failed'));
  const gone = lost.a ?? { state: 'disconnected' };
  expect(gone).toMatchObject({
    databases: [{ name: 'orders', state: 'failed', lost: true }, {}],
  });
  expect(sessionOf(gone, 'orders')).toBeNull();
  expect(statusTone(gone)).toBe('failed');
  expect(statusLabel(gone)).toBe('Connection lost');
});

it('keeps a database that failed to open from colouring the dot red', () => {
  const status = connected([
    { name: 'orders', state: 'failed', message: 'database does not exist' },
  ]);
  expect(statusTone(status)).toBe('connected');
});
