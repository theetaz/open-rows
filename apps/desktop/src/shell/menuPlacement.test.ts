// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { menuPlacement } from './menuPlacement';

const sidebar = { left: 64, right: 264 };

it('fits the database menu to its opener inside the sidebar', () => {
  const opener = { left: 72, right: 256, bottom: 90 };
  expect(menuPlacement(opener, sidebar, true)).toEqual({
    left: 72,
    top: 90,
    width: 184,
  });
});

it('never lets a menu overflow a narrow sidebar', () => {
  const actions = { left: 240, right: 256, bottom: 40 };
  const { left, width } = menuPlacement(actions, sidebar, false);
  expect(width).toBe(184);
  expect(left).toBe(72);
  expect(left + width).toBeLessThanOrEqual(sidebar.right - 8);
});

it('keeps the default width when there is room', () => {
  const opener = { left: 900, right: 920, bottom: 40 };
  const placed = menuPlacement(opener, { left: 0, right: 1280 }, false);
  expect(placed).toEqual({ left: 900, top: 40, width: 240 });
});
