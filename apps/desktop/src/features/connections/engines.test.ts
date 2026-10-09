// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import {
  ENGINE_CHOICES,
  defaultPort,
  engineName,
  matchingChoices,
  profileLocation,
} from './engines';
import { moveSelection } from './useEnginePicker';

it('lists every TablePlus connection type, six of them implemented', () => {
  expect(ENGINE_CHOICES).toHaveLength(21);
  const implemented = ENGINE_CHOICES.filter((choice) => choice.engine);
  expect(implemented.map((choice) => choice.engine)).toEqual([
    'postgres',
    'mysql',
    'mariadb',
    'sqlserver',
    'clickhouse',
    'sqlite',
  ]);
  expect(new Set(ENGINE_CHOICES.map((choice) => choice.id)).size).toBe(21);
  expect(engineName('mariadb')).toBe('MariaDB');
  expect(defaultPort('sqlite')).toBeNull();
  expect(defaultPort('sqlserver')).toBe(1433);
  expect(defaultPort('clickhouse')).toBe(8123);
  expect(engineName('sqlserver')).toBe('SQL Server');
});

it('searches by name, id or monogram', () => {
  expect(matchingChoices('sql').map((choice) => choice.id)).toEqual([
    'postgres',
    'mysql',
    'sqlserver',
    'libsql',
    'sqlite',
  ]);
  expect(matchingChoices('oracle sql')).toEqual([]);
  expect(matchingChoices('maria db').map((choice) => choice.id)).toEqual([
    'mariadb',
  ]);
  expect(matchingChoices('  ')).toHaveLength(21);
});

it('arrows move like Finder icon view and stop at the edges', () => {
  const all = ENGINE_CHOICES;
  expect(moveSelection(all, 'postgres', 'ArrowRight')).toBe('redshift');
  expect(moveSelection(all, 'postgres', 'ArrowDown')).toBe('cassandra');
  expect(moveSelection(all, 'postgres', 'ArrowLeft')).toBe('postgres');
  expect(moveSelection(all, 'vertica', 'ArrowDown')).toBe('vertica');
  expect(moveSelection(all, 'postgres', 'Enter')).toBe('postgres');
});

it('describes where a profile connects', () => {
  expect(profileLocation(profile('orders'))).toBe(
    'PostgreSQL · localhost:5432',
  );
  const file = profile('local', {
    engine: 'sqlite',
    file: { path: '/data/app.sqlite', readOnly: false, create: false },
  });
  expect(profileLocation(file)).toBe('SQLite · /data/app.sqlite');
});
