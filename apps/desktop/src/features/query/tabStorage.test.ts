// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { readSqlFile, saveSqlFile, sqlFileModified } from '@openrows/ipc';
import { stubStateKey } from '../../test/ipcStub';
import { bindFile, editTab, emptyTabs, openTab } from './tabs';
import { restoreTabs, serializeTabs } from './tabStorage';
import { useQueryTabs } from './useQueryTabs';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

afterEach(() => {
  vi.useRealTimers();
});

const scope = { connectionId: 'a', database: 'orders' };

it('stores query tabs with text, file and parameters, and table tabs', () => {
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
    ['table-2', 'users', ''],
    ['query-3', 'q.sql', 'SELECT :id'],
  ]);
  expect(restored.tabs[1]).toMatchObject({
    kind: 'table',
    table: { database: 'orders', schema: 'public', name: 'users' },
    objectPath: ['public', 'users'],
  });
  expect(restoreTabs(serializeTabs(state, false)).tabs).toHaveLength(1);
  expect(restored.tabs[2]?.file).toEqual({
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

/** Tabs as stored in the (stubbed) local store. */
const stored = (): string | null =>
  localStorage.getItem(stubStateKey('queryTabs'));

/** Mount the tab set and wait until its saved tabs loaded. */
async function mountTabs() {
  const hook = renderHook(() => useQueryTabs());
  await act(async () => {
    await Promise.resolve();
  });
  return hook;
}

it('auto saves tabs to the local store within a second and restores them on the next launch', async () => {
  vi.useFakeTimers();
  const first = await mountTabs();
  act(() => {
    first.result.current.open(scope);
  });
  act(() => {
    first.result.current.edit('query-1', 'SELECT 42');
  });
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
  expect(stored()).toContain('SELECT 42');
  first.unmount();
  vi.useRealTimers();
  const second = renderHook(() => useQueryTabs());
  await waitFor(() => {
    expect(second.result.current.activeFor(scope)?.sql).toBe('SELECT 42');
  });
});

it('imports tabs from web storage once, then clears it', async () => {
  const legacy = serializeTabs(openTab(emptyTabs, scope, { sql: 'SELECT 7' }));
  localStorage.setItem('openrows.queryTabs', legacy);
  const { result } = renderHook(() => useQueryTabs());
  await waitFor(() => {
    expect(result.current.activeFor(scope)?.sql).toBe('SELECT 7');
  });
  expect(stored()).toBe(legacy);
  expect(localStorage.getItem('openrows.queryTabs')).toBeNull();
});

it('offers Reload or Keep when another program changed a bound file', async () => {
  vi.mocked(sqlFileModified).mockResolvedValue(2);
  vi.mocked(readSqlFile).mockResolvedValue({
    file: { path: '/q.sql', name: 'q.sql', modifiedMs: 2 },
    sql: 'SELECT outside',
  });
  const { result } = await mountTabs();
  act(() => {
    result.current.open(scope, {
      sql: 'SELECT 1',
      file: { path: '/q.sql', name: 'q.sql', saved: 'SELECT 1', modifiedMs: 1 },
    });
  });
  act(() => {
    window.dispatchEvent(new Event('focus'));
  });
  await waitFor(() => {
    expect(result.current.activeFor(scope)?.file?.external).toBe(
      'SELECT outside',
    );
  });
  expect(result.current.activeFor(scope)?.file?.modifiedMs).toBe(2);
  expect(saveSqlFile).not.toHaveBeenCalled();
  vi.mocked(sqlFileModified).mockResolvedValue(null);
});

it('writes file-bound tabs back to their file, drops text without Auto Save and forgets tabs without restore', async () => {
  vi.useFakeTimers();
  const { result } = await mountTabs();
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
  expect(stored()).not.toContain('SELECT');
  localStorage.setItem(
    'openrows.general',
    JSON.stringify({ restoreSession: false }),
  );
  act(() => {
    result.current.edit('query-1', 'SELECT 3');
  });
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
  expect(stored()).toBeNull();
});
