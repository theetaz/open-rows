// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { QueryTab } from '../query/tabs';
import { moveImpact, parseTransfer, transferPayload } from './transfer';

const scope = { connectionId: 'id-a', database: 'app' };
const query: QueryTab = {
  ...scope,
  id: 'query-3',
  kind: 'query',
  title: 'Draft',
  sql: 'SELECT 1',
  params: { ':id': { kind: 'number', value: '7' } },
};
const table: QueryTab = {
  ...scope,
  id: 'table-4',
  kind: 'table',
  title: 'orders',
  sql: '',
  table: { database: 'app', schema: 'public', name: 'orders' },
  objectPath: ['public', 'orders'],
};
const monitor: QueryTab = {
  ...scope,
  id: 'monitor-5',
  kind: 'monitor',
  title: 'Monitor',
  sql: '',
};

it('moves descriptors with unsaved SQL, never results or parameter values', () => {
  const transfer = parseTransfer(transferPayload([query, table, monitor]));
  expect(transfer?.scope).toEqual(scope);
  expect(
    transfer?.tabs.tabs.map((tab) => [tab.kind, tab.title, tab.sql]),
  ).toEqual([
    ['query', 'Draft', 'SELECT 1'],
    ['table', 'orders', ''],
  ]);
  expect(transfer?.tabs.tabs[0]?.params).toBeUndefined();
  expect(transfer?.utilities).toEqual([{ ...scope, kind: 'monitor' }]);
});

it('rejects unusable payloads', () => {
  expect(parseTransfer('not json')).toBeNull();
  expect(parseTransfer('{"version":2,"tabs":"","utilities":[]}')).toBeNull();
  expect(parseTransfer('{"version":1,"tabs":7,"utilities":[]}')).toBeNull();
});

it('keeps journaled row edits, counts what cannot move, and waits for commits', () => {
  const staged = (count: number, pending = false) => ({
    current: Array.from({ length: count }),
    pending,
  });
  const entry = (rows: number, schema: number, pending = false) =>
    ({ edits: staged(rows, pending), schema: staged(schema) }) as never;
  expect(moveImpact(table, entry(2, 1))).toEqual({
    pending: false,
    journaled: 2,
    lost: 1,
  });
  expect(moveImpact(query, entry(3, 0))).toEqual({
    pending: false,
    journaled: 0,
    lost: 3,
  });
  expect(moveImpact(table, entry(1, 0, true)).pending).toBe(true);
});
