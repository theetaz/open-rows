// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import {
  insertHistorySql,
  insertSavedSql,
  useHistoryInsert,
} from './historyInsert';
import type { EditorHandle } from './request';
it('inserts history at the cursor of the last focused split pane without executing', () => {
  const first = {
    current: {
      text: () => 'SELECT tail',
      cursor: () => 7,
      replace: vi.fn(),
      focus: vi.fn(),
    } as unknown as EditorHandle,
  };
  const second = {
    current: {
      text: () => 'second',
      cursor: () => 0,
      replace: vi.fn(),
      focus: vi.fn(),
    } as unknown as EditorHandle,
  };
  const hook = renderHook(() => {
    const claim = useHistoryInsert('first', first);
    useHistoryInsert('second', second);
    return claim;
  });
  act(() => {
    hook.result.current();
  });
  expect(insertHistorySql('history ')).toBe(true);
  expect(first.current.replace).toHaveBeenCalledWith('SELECT history tail');
  expect(second.current.replace).not.toHaveBeenCalled();
  expect(first.current.focus).toHaveBeenCalled();
  hook.unmount();
  expect(insertHistorySql('unused')).toBe(false);
});

it('inserts saved SQL literally over the focused editor selection', () => {
  const insertText = vi.fn();
  const current = { insertText } as unknown as EditorHandle;
  const hook = renderHook(() => useHistoryInsert('saved', { current }));
  act(() => {
    hook.result.current();
  });
  expect(insertSavedSql("SELECT '${1:literal}', :id;")).toBe(true);
  expect(insertText).toHaveBeenCalledWith("SELECT '${1:literal}', :id;");
  hook.unmount();
  expect(insertSavedSql('SELECT 1')).toBe(false);
});
