// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { EditInfoDto } from '@openrows/ipc';
import { mappingReason, resultMapping } from './resultMapping';

const columns = ['id', 'name'].map((name) => ({ name, sourceType: 'text' }));
const info: EditInfoDto = {
  key: ['id'],
  protected: [],
  rowId: null,
  readOnly: false,
  confirmWrites: true,
  production: false,
  columns: columns.map(({ name }) => ({
    name,
    dataType: 'text',
    nullable: false,
    primaryKey: name === 'id',
    default: null,
    comment: null,
    enumLabels: null,
  })),
};
it.each([
  'SELECT * FROM public.users',
  'SELECT u.id, u.name FROM public.users AS u WHERE u.id > 0 ORDER BY name LIMIT 20;',
  '-- selection\nSELECT "id", "name" FROM "public"."users" WHERE name = \'DISTINCT JOIN\'',
  'SELECT ALL id, name FROM public.users OFFSET 2',
])('accepts a direct keyed base-table projection: %s', (sql) => {
  const mapping = resultMapping(sql, columns, 'postgres');
  expect(mapping).toMatchObject({
    kind: 'table',
    schema: 'public',
    table: 'users',
  });
  expect(mappingReason(mapping, info)).toBeNull();
});
it.each([
  'SELECT DISTINCT id, name FROM public.users',
  'SELECT id, name FROM public.users JOIN public.other USING(id)',
  'SELECT id, name FROM public.users, public.other',
  'SELECT id, count(*) FROM public.users GROUP BY id',
  'SELECT id, upper(name) FROM public.users',
  'SELECT id, name AS id FROM public.users',
  'WITH x AS (SELECT * FROM public.users) SELECT * FROM x',
  'SELECT * FROM (SELECT * FROM public.users) u',
  'SELECT * FROM public.users UNION SELECT * FROM public.other',
  'SELECT * FROM public.users; DELETE FROM public.users',
  'SELECT * INTO public.copy FROM public.users',
  'SELECT * FROM public.users WHERE id IN (SELECT id FROM public.other)',
  'SELECT * FROM public.users /* unsupported comment */',
  'SELECT * FROM public.users FOR UPDATE',
  'SELECT id, current_user FROM public.users',
  'SELECT id, name FROM public.users u(id, name)',
])('refuses unsafe or unsupported provenance with a reason: %s', (sql) => {
  const mapping = resultMapping(sql, columns, 'postgres');
  expect(mapping.kind).toBe('readonly');
  if (mapping.kind === 'readonly') expect(mapping.reason).not.toBe('');
});
it('does not guess the schema from a default search path', () => {
  const mapping = resultMapping('SELECT * FROM users', columns, 'postgres');
  expect(mapping.kind).toBe('readonly');
  if (mapping.kind === 'readonly') expect(mapping.reason).toContain('Qualify');
});
it('rejects duplicate labels and result/projection mismatch', () => {
  expect(
    resultMapping(
      'SELECT * FROM public.users',
      [
        { name: 'id', sourceType: null },
        { name: 'id', sourceType: null },
      ],
      'postgres',
    ),
  ).toMatchObject({ kind: 'readonly' });
  expect(
    resultMapping('SELECT name, id FROM public.users', columns, 'postgres'),
  ).toMatchObject({ kind: 'readonly' });
});
it('requires all composite key columns, a catalog match and a writable connection', () => {
  const mapping = resultMapping(
    'SELECT * FROM public.users',
    columns,
    'postgres',
  );
  expect(mappingReason(mapping, { ...info, key: [] })).toMatch(/no supported/);
  expect(mappingReason(mapping, { ...info, key: ['id', 'tenant'] })).toMatch(
    /every.*key/,
  );
  expect(mappingReason(mapping, { ...info, readOnly: true })).toMatch(
    /read-only/,
  );
  expect(mappingReason(mapping, { ...info, columns: [] })).toMatch(
    /not a column/,
  );
  expect(mappingReason(mapping, { ...info, rowId: 'rowid' })).toMatch(
    /no supported/,
  );
  expect(mappingReason(mapping, null)).toMatch(/Checking/);
});
it('recognizes a non-null unique key supplied by the backend and quoted identifiers', () => {
  const mapping = resultMapping(
    'SELECT * FROM "odd.schema"."some""table"',
    columns,
    'postgres',
  );
  expect(mapping).toMatchObject({ schema: 'odd.schema', table: 'some"table' });
  expect(mappingReason(mapping, { ...info, key: ['name'] })).toBeNull();
});
