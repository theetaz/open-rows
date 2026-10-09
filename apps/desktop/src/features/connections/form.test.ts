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
  urlConflict,
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
    engine: 'postgres',
    file: null,
    name: 'orders',
    group: 'Team',
    env: { kind: 'local', label: null, color: null },
    host: 'localhost',
    port: 5432,
    database: null,
    user: 'app',
    tlsMode: 'prefer',
    tlsFiles: {
      caFile: null,
      cloudCa: null,
      clientCert: null,
      clientKey: null,
      keyPassphrase: { kind: 'keep' },
    },
    ssh: null,
    proxy: null,
    safety: { readOnly: false, confirmWrites: false },
    recordHistory: true,
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
    engine: 'postgres' as const,
    file: null,
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

it('fills a file connection from a sqlite: URL and validates its path', () => {
  const parsed = {
    engine: 'sqlite' as const,
    file: { path: '/data/shop.sqlite', readOnly: true, create: false },
    host: 'localhost',
    port: 5432,
    database: null,
    user: null,
    tlsMode: 'disable' as const,
    passwordProvided: false,
  };
  const draft = withUrl(emptyDraft(), parsed);
  expect(draft).toMatchObject({
    engine: 'sqlite',
    name: 'shop',
    filePath: '/data/shop.sqlite',
    fileReadOnly: true,
  });
  expect(validate(draft)).toEqual({});
  expect(validate({ ...draft, filePath: 'shop.sqlite' }).filePath).toBe(
    'Use an absolute path',
  );
  expect(validate({ ...draft, filePath: ' ' }).filePath).toBe(
    'Choose a database file',
  );
  const input = toInput(draft);
  expect(input.file).toEqual({
    path: '/data/shop.sqlite',
    memory: false,
    readOnly: true,
    create: false,
  });
  expect(input.ssh).toBeNull();
  expect(input.password).toEqual({ kind: 'clear' });
});

it('an in-memory sqlite: URL needs no path and keeps the typed one', () => {
  const parsed = {
    engine: 'sqlite' as const,
    file: { path: '', memory: true, readOnly: false, create: false },
    host: 'localhost',
    port: 5432,
    database: null,
    user: null,
    tlsMode: 'disable' as const,
    passwordProvided: false,
  };
  const typed = { ...emptyDraft('sqlite'), filePath: '/data/shop.sqlite' };
  const draft = withUrl({ ...typed, fileReadOnly: true }, parsed);
  expect(draft).toMatchObject({
    name: 'In-memory',
    fileMemory: true,
    filePath: '/data/shop.sqlite',
  });
  expect(validate({ ...draft, filePath: '' })).toEqual({});
  expect(toInput(draft).file).toEqual({
    path: '',
    memory: true,
    readOnly: false,
    create: false,
  });
  const back = withUrl(draft, {
    ...parsed,
    file: { path: '/data/b.sqlite', readOnly: false, create: false },
  });
  expect(back).toMatchObject({ fileMemory: false, filePath: '/data/b.sqlite' });
});

it('maps a saved in-memory profile to the in-memory switch', () => {
  const stored = profile('scratch', {
    engine: 'sqlite',
    file: { path: '', memory: true, readOnly: false, create: false },
  });
  expect(draftFromProfile(stored).fileMemory).toBe(true);
});

it('starts new connections with the engine default port', () => {
  expect(emptyDraft('mysql').port).toBe('3306');
  expect(emptyDraft('mariadb').port).toBe('3306');
  expect(emptyDraft('postgres').port).toBe('5432');
  expect(emptyDraft('sqlite').port).toBe('');
});

it('a saved connection refuses a URL for another engine', () => {
  const draft = draftFromProfile(profile('orders'));
  const mysql = {
    engine: 'mysql' as const,
    file: null,
    host: 'db',
    port: 3306,
    database: 'shop',
    user: 'app',
    tlsMode: 'prefer' as const,
    passwordProvided: false,
  };
  expect(urlConflict(draft, mysql)).toBe(
    'This is a MySQL URL; the connection uses PostgreSQL',
  );
  expect(urlConflict(emptyDraft(), mysql)).toBeNull();
  expect(withUrl(emptyDraft(), mysql)).toMatchObject({
    engine: 'mysql',
    port: '3306',
    database: 'shop',
  });
});
