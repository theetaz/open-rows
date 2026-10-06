// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { TextCache } from './text';
it('bounds long-text measurement logarithmically and reuses prepared strings', () => {
  const measureText = vi.fn((value: string) => ({ width: value.length }));
  const context = { measureText } as unknown as CanvasRenderingContext2D;
  const cache = new TextCache();
  const value = { text: 'x'.repeat(4096), state: 'normal' as const };
  cache.prepare(context, value, 16);
  expect(cache.get(value)).toBe('x'.repeat(15) + '…');
  expect(measureText.mock.calls.length).toBeLessThanOrEqual(14);
  measureText.mockClear();
  cache.prepare(context, value, 16);
  expect(measureText).not.toHaveBeenCalled();
});
