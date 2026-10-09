// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { objectTab } from './queryCommands';

const ref = {
  database: 'orders',
  schema: 'my "app"',
  name: 'Order Items',
  kind: 'table' as const,
  arguments: null,
};

it('selects relations with quoted identifiers and opens routines empty', () => {
  expect(objectTab(ref)).toEqual({
    title: 'Order Items',
    objectPath: ['my "app"', 'Order Items'],
    sql: 'SELECT * FROM "my ""app"""."Order Items" LIMIT 1000;',
  });
  expect(objectTab({ ...ref, kind: 'view' }).sql).toMatch(/^SELECT \*/);
  expect(
    objectTab({ ...ref, kind: 'function', arguments: 'total numeric' }),
  ).toEqual({
    title: 'Order Items(total numeric)',
    objectPath: ['my "app"', 'Order Items(total numeric)'],
    sql: '',
  });
});
