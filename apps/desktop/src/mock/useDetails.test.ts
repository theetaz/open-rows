// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { useDetails } from './useDetails';

it('reveals Details for a newly selected row but not for staged refreshes', () => {
  const setPanel = vi.fn();
  const hook = renderHook(() => useDetails(setPanel, 'tab'));
  const row = { table: 'users', number: 1, fields: [] };
  act(() => {
    hook.result.current.onRow(row);
  });
  expect(setPanel).toHaveBeenCalledWith('Details');
  setPanel.mockClear();
  act(() => {
    hook.result.current.onRow({ ...row, number: 2 }, false);
  });
  expect(setPanel).not.toHaveBeenCalled();
  expect(hook.result.current.row?.number).toBe(2);
});
