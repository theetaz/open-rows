// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { logicalOffset, nativeOffset, spacerHeight } from './scroll';

it('keeps normal result scroll coordinates unchanged', () => {
  expect(spacerHeight(26_000_026)).toBe(26_000_026);
  expect(logicalOffset(13_000_000, 800, 26_000_026)).toBe(13_000_000);
  expect(nativeOffset(13_000_000, 800, 26_000_026)).toBe(13_000_000);
});
it('maps a capped native scrollbar to the last of five million rows', () => {
  const content = 5_000_001 * 26;
  const viewport = 800;
  const end = spacerHeight(content) - viewport;
  expect(spacerHeight(content)).toBe(32_000_000);
  expect(logicalOffset(end, viewport, content)).toBe(content - viewport);
  expect(nativeOffset(content - viewport, viewport, content)).toBeCloseTo(end);
  const middle = nativeOffset(65_000_000, viewport, content);
  expect(logicalOffset(middle, viewport, content)).toBeCloseTo(65_000_000);
});
it('has no scroll range when content fits the viewport', () => {
  expect(nativeOffset(0, 800, 26)).toBe(0);
  expect(logicalOffset(0, 800, 26)).toBe(0);
});
