// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it } from 'vitest';
import { useDetails } from './useDetails';

it('shows the latest selection and hides it on another tab', () => {
  const hook = renderHook(({ scope }) => useDetails(scope), {
    initialProps: { scope: 'tab' },
  });
  const row = { table: 'users', fields: [] };
  act(() => {
    hook.result.current.onRow(row);
  });
  expect(hook.result.current.row).toBe(row);
  hook.rerender({ scope: 'other' });
  expect(hook.result.current.row).toBeNull();
});
