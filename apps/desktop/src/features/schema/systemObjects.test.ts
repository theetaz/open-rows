// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { isSystemObject } from './systemObjects';
import type { EngineDto } from '@openrows/ipc';

it.each<[EngineDto, string[]]>([
  [
    'postgres',
    ['pg_catalog', 'information_schema', 'pg_toast', 'pg_toast_temp_1'],
  ],
  ['mysql', ['mysql', 'sys', 'performance_schema', 'information_schema']],
  ['mariadb', ['mysql', 'sys', 'performance_schema', 'information_schema']],
  ['sqlserver', ['sys', 'INFORMATION_SCHEMA']],
  ['clickhouse', ['system', 'INFORMATION_SCHEMA', 'information_schema']],
])(
  '%s classifies system schemas without hiding a user schema',
  (engine, schemas) => {
    for (const schema of schemas)
      expect(isSystemObject(engine, { schema, name: 'objects' })).toBe(true);
    expect(isSystemObject(engine, { schema: 'app', name: 'objects' })).toBe(
      false,
    );
  },
);

it('SQLite hides sqlite_% objects rather than all objects in main', () => {
  expect(
    isSystemObject('sqlite', { schema: 'main', name: 'sqlite_sequence' }),
  ).toBe(true);
  expect(
    isSystemObject('sqlite', { schema: 'main', name: 'sqlite_stat1' }),
  ).toBe(true);
  expect(isSystemObject('sqlite', { schema: 'main', name: 'sqliteX' })).toBe(
    false,
  );
  expect(isSystemObject('sqlite', { schema: 'main', name: 'orders' })).toBe(
    false,
  );
});

it.each(['libsql', 'libsql-file'] as const)(
  '%s hides internal tables while retaining user objects',
  (engine) => {
    for (const name of [
      'sqlite_sequence',
      '_litestream_seq',
      'libsql_stats',
      'LIBSQL_STATS',
    ])
      expect(isSystemObject(engine, { schema: 'main', name })).toBe(true);
    for (const name of ['orders', 'sqliteX', 'libsqlX', '_litestreamX'])
      expect(isSystemObject(engine, { schema: 'main', name })).toBe(false);
  },
);
