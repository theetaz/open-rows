// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  closeTab,
  editTab,
  emptyTabs,
  openTab,
  scopeKey,
  selectTab,
  tabsOf,
} from './tabs';
import { runRequest } from './request';
import { restoreTabs, serializeTabs } from './tabStorage';

it('keeps tab sets and active tabs per connection and database', () => {
  const a = { connectionId: 'a', database: '' };
  const b = { connectionId: 'b', database: '' };
  const reports = { connectionId: 'a', database: 'reports' };
  let state = openTab(emptyTabs, a);
  state = openTab(state, b, {
    title: 'orders',
    objectPath: ['public', 'orders'],
  });
  state = openTab(state, a, { sql: 'SELECT 1' });
  state = openTab(state, reports);
  expect(tabsOf(state, a).map((tab) => tab.title)).toEqual([
    'SQL Query 1',
    'SQL Query 3',
  ]);
  expect(tabsOf(state, reports).map((tab) => tab.id)).toEqual(['query-4']);
  expect(state.active).toEqual({
    [scopeKey(a)]: 'query-3',
    [scopeKey(b)]: 'query-2',
    [scopeKey(reports)]: 'query-4',
  });
  state = selectTab(state, 'query-1');
  state = editTab(state, 'query-1', 'SELECT 2');
  expect(tabsOf(state, a)[0]?.sql).toBe('SELECT 2');
  state = closeTab(state, 'query-1');
  expect(state.active[scopeKey(a)]).toBe('query-3');
  state = closeTab(state, 'query-3');
  expect(state.active[scopeKey(a)]).toBeUndefined();
  expect(state.active[scopeKey(reports)]).toBe('query-4');
  expect(tabsOf(state, b)[0]?.objectPath).toEqual(['public', 'orders']);
  expect(closeTab(state, 'missing')).toBe(state);
});

it('builds run requests for current, selection and all, with the row limit', () => {
  const editor = (text: string, cursor: number, selection = '') => ({
    text: () => text,
    cursor: () => cursor,
    selection: () => selection,
    focus: () => undefined,
    format: () => Promise.resolve(),
    replace: () => undefined,
    insertSnippet: () => undefined,
    runStart: () => 0,
  });
  expect(runRequest(editor('SELECT 1', 3), 'current', 's', null)).toEqual({
    sessionId: 's',
    sql: 'SELECT 1',
    mode: 'current',
    cursor: 3,
    confirmToken: null,
    maxRows: null,
  });
  expect(
    runRequest(
      editor('SELECT 1; SELECT 2', 3, 'SELECT 2'),
      'current',
      's',
      500,
    ),
  ).toMatchObject({
    sql: 'SELECT 2',
    mode: 'selection',
    cursor: null,
    maxRows: 500,
  });
  expect(
    runRequest(editor('SELECT 1', 3, 'SELECT'), 'all', 's', 1_000),
  ).toMatchObject({ sql: 'SELECT 1', mode: 'all', maxRows: 1_000 });
  expect(runRequest(editor('  \n', 0), 'all', 's', null)).toBeNull();
});

it('opens one processes tab per scope, refocusing it, and never stores it', () => {
  const orders = { connectionId: 'a', database: 'orders' };
  const reports = { connectionId: 'a', database: 'reports' };
  let state = openTab(emptyTabs, orders, {
    processes: true,
    title: 'Processes',
  });
  state = openTab(state, orders);
  state = openTab(state, orders, { processes: true, title: 'Processes' });
  state = openTab(state, reports, { processes: true, title: 'Processes' });
  expect(tabsOf(state, orders).map((tab) => [tab.id, tab.kind])).toEqual([
    ['processes-1', 'processes'],
    ['query-2', 'query'],
  ]);
  expect(state.active[scopeKey(orders)]).toBe('processes-1');
  expect(tabsOf(state, reports)).toHaveLength(1);
  const stored = restoreTabs(serializeTabs(state));
  expect(stored.tabs.map((tab) => tab.id)).toEqual(['query-2']);
});

it('opens one lazy monitor per scope and never restores collection from saved tabs', () => {
  const scope = { connectionId: 'p', database: 'db' };
  let state = openTab(emptyTabs, scope, { monitor: true, title: 'Monitor' });
  state = openTab(state, scope, { monitor: true, title: 'Monitor' });
  expect(tabsOf(state, scope)).toHaveLength(1);
  expect(tabsOf(state, scope)[0]?.kind).toBe('monitor');
  expect(restoreTabs(serializeTabs(state)).tabs).toHaveLength(0);
});
