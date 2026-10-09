// SPDX-License-Identifier: Apache-2.0
import { renderHook } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { checkCatalog } from '@openrows/ipc';
import { useSchemaFingerprint } from './useSchemaFingerprint';

vi.mock('@openrows/ipc', () => ({
  checkCatalog: vi.fn(() => Promise.resolve(false)),
}));

beforeEach(() => {
  vi.mocked(checkCatalog).mockClear();
});

it('checks on mount and on focus, at most once per interval', () => {
  const now = vi.spyOn(performance, 'now').mockReturnValue(1000);
  const { unmount } = renderHook(() => {
    useSchemaFingerprint('p', 'orders');
  });
  expect(checkCatalog).toHaveBeenCalledWith('p', 'orders');
  window.dispatchEvent(new Event('focus'));
  expect(checkCatalog).toHaveBeenCalledTimes(1);
  now.mockReturnValue(7000);
  window.dispatchEvent(new Event('focus'));
  expect(checkCatalog).toHaveBeenCalledTimes(2);
  unmount();
  now.mockReturnValue(20_000);
  window.dispatchEvent(new Event('focus'));
  expect(checkCatalog).toHaveBeenCalledTimes(2);
  now.mockRestore();
});

it('does nothing without a connected database', () => {
  renderHook(() => {
    useSchemaFingerprint(null, '');
  });
  window.dispatchEvent(new Event('focus'));
  expect(checkCatalog).not.toHaveBeenCalled();
});
