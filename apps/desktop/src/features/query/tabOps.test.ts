// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { emptyTabs, openTab, tabsOf } from './tabs';
import type { QueryTabInit, TabsState } from './tabs';
import { keepTab, moveTab, otherTabs, pinTab, tabsRightOf } from './tabOps';

const scope = { connectionId: 'a', database: 'orders' };
const other = { connectionId: 'b', database: 'orders' };
const table = (name: string): QueryTabInit => ({
  title: name,
  table: { database: 'orders', schema: 'public', name },
});

/** Four query tabs in `scope` with one tab of another scope in between. */
function four(): TabsState {
  let state = openTab(emptyTabs, scope);
  state = openTab(state, other);
  for (let index = 0; index < 3; index += 1) state = openTab(state, scope);
  return state;
}

const ids = (state: TabsState): string[] =>
  tabsOf(state, scope).map((tab) => tab.id);

it('reorders within a scope and leaves other scopes in place', () => {
  const state = moveTab(four(), 'query-5', 0);
  expect(ids(state)).toEqual(['query-5', 'query-1', 'query-3', 'query-4']);
  expect(state.tabs[1]?.id).toBe('query-2');
  expect(ids(moveTab(state, 'query-5', 99))).toEqual([
    'query-1',
    'query-3',
    'query-4',
    'query-5',
  ]);
});

it('keeps pinned tabs first and out of Close Others and Close to the Right', () => {
  let state = pinTab(four(), 'query-4', true);
  expect(ids(state)).toEqual(['query-4', 'query-1', 'query-3', 'query-5']);
  expect(otherTabs(state, 'query-3')).toEqual(['query-1', 'query-5']);
  expect(tabsRightOf(state, 'query-4')).toEqual([
    'query-1',
    'query-3',
    'query-5',
  ]);
  expect(tabsRightOf(state, 'query-3')).toEqual(['query-5']);
  expect(ids(moveTab(state, 'query-5', 0))[0]).toBe('query-4');
  state = pinTab(state, 'query-4', false);
  expect(state.tabs.find((tab) => tab.id === 'query-4')?.pinned).toBe(false);
});

it('replaces the preview tab in place until it is kept', () => {
  let state = openTab(emptyTabs, scope);
  state = openTab(state, scope, { ...table('users'), preview: true });
  state = openTab(state, scope);
  state = openTab(state, scope, { ...table('orders'), preview: true });
  expect(tabsOf(state, scope).map((tab) => tab.title)).toEqual([
    'SQL Query 1',
    'orders',
    'SQL Query 3',
  ]);
  expect(state.active['a\norders']).toBe('table-4');
  state = keepTab(state, 'table-4');
  state = openTab(state, scope, { ...table('items'), preview: true });
  expect(tabsOf(state, scope).map((tab) => tab.title)).toEqual([
    'SQL Query 1',
    'orders',
    'SQL Query 3',
    'items',
  ]);
  expect(keepTab(state, 'query-1')).toBe(state);
});
