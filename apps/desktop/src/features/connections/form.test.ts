// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { profile } from '../../test/ipcStub';
import {
  draftFromProfile,
  emptyDraft,
  toInput,
  validate,
  withEnvironment,
  withPassword,
  withPatch,
  withUrl,
} from './form';

vi.mock('@openrows/ipc', () => ({ DEFAULT_CUSTOM_ENV_COLOR: '#7c3aed' }));

it('maps a profile to a draft and back without inventing a password', () => {
  const stored = profile('orders', {
    group: 'Team',
    hasPassword: true,
    database: null,
  });
  const input = toInput(draftFromProfile(stored));
  expect(input).toEqual({
    name: 'orders',
    group: 'Team',
    env: { kind: 'local', label: null, color: null },
    host: 'localhost',
    port: 5432,
    database: null,
    user: 'app',
    tlsMode: 'prefer',
    safety: { readOnly: false, confirmWrites: false },
    password: { kind: 'keep' },
  });
});

it('prod turns confirm writes on; leaving prod keeps the choice', () => {
  const prod = withEnvironment(emptyDraft(), 'prod');
  expect(prod.confirmWrites).toBe(true);
  expect(withEnvironment(prod, 'dev').confirmWrites).toBe(true);
  expect(withEnvironment(emptyDraft(), 'staging').confirmWrites).toBe(false);
});

it('fills fields from a parsed URL; a typed name sticks, a URL-derived one follows', () => {
  const parsed = {
    host: 'db',
    port: 5433,
    database: 'orders',
    user: 'app',
    tlsMode: 'require' as const,
    passwordProvided: false,
  };
  const draft = withUrl(emptyDraft(), parsed);
  expect(draft).toMatchObject({ name: 'orders', host: 'db', port: '5433' });
  expect(withUrl(draft, { ...parsed, database: 'sales' }).name).toBe('sales');
  const named = withPatch(draft, { name: 'Mine' });
  expect(withUrl(named, { ...parsed, database: 'sales' }).name).toBe('Mine');
  const typed = withPassword(draft, 'typed');
  expect(toInput(typed).password).toEqual({ kind: 'set', value: 'typed' });
  expect(toInput(withPassword(typed, '')).password).toEqual({ kind: 'keep' });
});

it('validates name, host, port range, user and custom label', () => {
  const draft = {
    ...emptyDraft(),
    host: ' ',
    port: '0',
    envKind: 'custom' as const,
  };
  expect(Object.keys(validate(draft)).sort()).toEqual([
    'envLabel',
    'host',
    'name',
    'port',
    'user',
  ]);
  const custom = toInput({ ...draft, envLabel: ' qa ', envColor: '#010203' });
  expect(custom.env).toEqual({ kind: 'custom', label: 'qa', color: '#010203' });
});
