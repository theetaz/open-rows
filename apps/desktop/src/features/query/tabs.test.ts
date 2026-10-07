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
import { runRequest, runScopeOf } from './request';

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

it('builds run requests for current, selection and all', () => {
  const editor = (text: string, cursor: number, selection = '') => ({
    text: () => text,
    cursor: () => cursor,
    selection: () => selection,
    focus: () => undefined,
  });
  expect(runRequest(editor('SELECT 1', 3), 'current', 's')).toEqual({
    sessionId: 's',
    sql: 'SELECT 1',
    mode: 'current',
    cursor: 3,
    confirmToken: null,
  });
  expect(
    runRequest(editor('SELECT 1; SELECT 2', 3, 'SELECT 2'), 'current', 's'),
  ).toMatchObject({
    sql: 'SELECT 2',
    mode: 'selection',
    cursor: null,
  });
  expect(runRequest(editor('SELECT 1', 3, 'SELECT'), 'all', 's')).toMatchObject(
    {
      sql: 'SELECT 1',
      mode: 'all',
    },
  );
  expect(runRequest(editor('  \n', 0), 'all', 's')).toBeNull();
});

it('maps ⌘↵ and ⇧⌘↵ but not handled or modified keys', () => {
  const key = {
    key: 'Enter',
    metaKey: true,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    defaultPrevented: false,
  };
  expect(runScopeOf(key)).toBe('current');
  expect(runScopeOf({ ...key, shiftKey: true })).toBe('all');
  expect(runScopeOf({ ...key, metaKey: false, ctrlKey: true })).toBe('current');
  expect(runScopeOf({ ...key, metaKey: false })).toBeNull();
  expect(runScopeOf({ ...key, altKey: true })).toBeNull();
  expect(runScopeOf({ ...key, defaultPrevented: true })).toBeNull();
  expect(runScopeOf({ ...key, key: 'a' })).toBeNull();
});
