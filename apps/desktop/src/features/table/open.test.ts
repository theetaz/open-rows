// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { QueryTabs } from '../query';
import type { QueryTab } from '../query/tabs';
import { openTableView } from './open';

const scope = { connectionId: 'c', database: '' };
const users = {
  database: 'orders',
  schema: 'public',
  name: 'users',
  kind: 'table' as const,
};

function tabs(open: QueryTab[] = []): QueryTabs {
  return {
    tabsFor: () => open,
    activeFor: () => undefined,
    open: vi.fn(),
    close: vi.fn(),
    select: vi.fn(),
    edit: vi.fn(),
  };
}

it('opens relations as table tabs titled with their name', () => {
  for (const kind of [
    'table',
    'view',
    'materializedView',
    'foreignTable',
  ] as const) {
    const target = tabs();
    expect(openTableView(target, scope, { ...users, kind })).toBe(true);
    expect(target.open).toHaveBeenCalledWith(scope, {
      title: 'users',
      objectPath: ['public', 'users'],
      table: { database: 'orders', schema: 'public', name: 'users' },
    });
  }
});

it('refocuses an open table tab instead of opening another', () => {
  const open: QueryTab = {
    ...scope,
    id: 'table-3',
    kind: 'table',
    title: 'users',
    sql: '',
    table: { database: 'orders', schema: 'public', name: 'users' },
  };
  const target = tabs([open]);
  expect(openTableView(target, scope, users)).toBe(true);
  expect(target.select).toHaveBeenCalledWith('table-3');
  expect(target.open).not.toHaveBeenCalled();
});

it('leaves routines and sequences to query tabs', () => {
  const target = tabs();
  expect(openTableView(target, scope, { ...users, kind: 'function' })).toBe(
    false,
  );
  expect(openTableView(target, scope, { ...users, kind: 'sequence' })).toBe(
    false,
  );
  expect(target.open).not.toHaveBeenCalled();
});
