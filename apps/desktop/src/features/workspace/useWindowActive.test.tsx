// SPDX-License-Identifier: Apache-2.0
import { fireEvent, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { useWindowActive } from './useWindowActive';

it('tracks focus changes and removes listeners and its root state on unmount', () => {
  const focused = vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  const root = document.documentElement;
  delete root.dataset.windowActive;
  const { unmount } = renderHook(useWindowActive);
  expect(root.dataset.windowActive).toBe('true');
  fireEvent(window, new Event('blur'));
  expect(root.dataset.windowActive).toBe('false');
  fireEvent(window, new Event('focus'));
  expect(root.dataset.windowActive).toBe('true');
  unmount();
  expect(root.dataset.windowActive).toBeUndefined();
  fireEvent(window, new Event('blur'));
  expect(root.dataset.windowActive).toBeUndefined();
  focused.mockRestore();
});
