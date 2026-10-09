// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { saveSqlFile } from '@openrows/ipc';
import { bindFile, editTab, emptyTabs, openTab } from './tabs';
import { TABS_KEY, restoreTabs, serializeTabs } from './tabStorage';
import { useQueryTabs } from './useQueryTabs';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

afterEach(() => {
  vi.useRealTimers();
});

const scope = { connectionId: 'a', database: 'orders' };

it('stores query tabs with text, file and parameters, never table tabs', () => {
  let state = openTab(emptyTabs, scope, { sql: 'SELECT 1' });
  state = openTab(state, scope, {
    title: 'users',
    table: { database: 'orders', schema: 'public', name: 'users' },
  });
  state = openTab(state, scope);
  state = bindFile(state, 'query-3', {
    path: '/q.sql',
    name: 'q.sql',
    saved: '',
  });
  state = editTab(state, 'query-3', 'SELECT :id');
  const restored = restoreTabs(serializeTabs(state));
  expect(restored.tabs.map((tab) => [tab.id, tab.title, tab.sql])).toEqual([
    ['query-1', 'SQL Query 1', 'SELECT 1'],
    ['query-3', 'q.sql', 'SELECT :id'],
  ]);
  expect(restored.tabs[1]?.file).toEqual({
    path: '/q.sql',
    name: 'q.sql',
    saved: '',
  });
  expect(restored.active).toEqual({ 'a\norders': 'query-3' });
  expect(restored.next).toBe(4);
});

it('falls back to a scope’s last tab and ignores unreadable data', () => {
  const text = JSON.stringify({
    tabs: [
      {
        id: 'query-7',
        kind: 'query',
        connectionId: 'a',
        database: 'orders',
        title: 't',
        sql: '',
      },
      { id: 'x', kind: 'query', title: 3 },
    ],
    active: { 'a\norders': 'table-2' },
    next: 2,
  });
  const restored = restoreTabs(text);
  expect(restored.tabs.map((tab) => tab.id)).toEqual(['query-7']);
  expect(restored.active).toEqual({ 'a\norders': 'query-7' });
  expect(restored.next).toBe(8);
  expect(restoreTabs('{').tabs).toEqual([]);
  expect(restoreTabs(null).tabs).toEqual([]);
});

it('auto saves tabs within a second and restores them on the next launch', () => {
  vi.useFakeTimers();
  const first = renderHook(() => useQueryTabs());
  act(() => {
    first.result.current.open(scope);
  });
  act(() => {
    first.result.current.edit('query-1', 'SELECT 42');
  });
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
  expect(localStorage.getItem(TABS_KEY)).toContain('SELECT 42');
  first.unmount();
  const second = renderHook(() => useQueryTabs());
  expect(second.result.current.activeFor(scope)?.sql).toBe('SELECT 42');
});

it('writes file-bound tabs back to their file and forgets tabs when off', async () => {
  vi.useFakeTimers();
  const { result } = renderHook(() => useQueryTabs());
  act(() => {
    result.current.open(scope);
  });
  act(() => {
    result.current.bindFile('query-1', {
      path: '/q.sql',
      name: 'q.sql',
      saved: '',
    });
    result.current.edit('query-1', 'SELECT 1');
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1_000);
  });
  expect(saveSqlFile).toHaveBeenCalledWith('/q.sql', 'SELECT 1');
  expect(result.current.activeFor(scope)?.file?.saved).toBe('SELECT 1');
  localStorage.setItem('openrows.editor', JSON.stringify({ autoSave: false }));
  act(() => {
    result.current.edit('query-1', 'SELECT 2');
  });
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
  expect(saveSqlFile).toHaveBeenCalledTimes(1);
  expect(localStorage.getItem(TABS_KEY)).toBeNull();
});
