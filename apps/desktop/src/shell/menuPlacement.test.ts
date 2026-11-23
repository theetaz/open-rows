// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { clampToWindow, menuPlacement } from './menuPlacement';

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

const view = { width: 1000, height: 700 };
const menu = { width: 240, height: 200 };

it('keeps a menu 8 px inside every window edge', () => {
  expect(clampToWindow({ ...menu, x: 900, y: 100 }, view)).toEqual({
    left: 752,
    top: 100,
    maxHeight: null,
  });
  expect(clampToWindow({ ...menu, x: -20, y: 100 }, view).left).toBe(8);
});

it('opens upward when there is no room below', () => {
  const placed = clampToWindow({ ...menu, x: 300, y: 600, flip: 590 }, view);
  expect(placed).toEqual({ left: 300, top: 390, maxHeight: null });
});

it('opens an above menu downward when there is no room above', () => {
  const placed = clampToWindow({ ...menu, x: 300, y: 120, above: true }, view);
  expect(placed.top).toBe(120);
});

it('pins a menu that fits neither side 8 px from the bottom', () => {
  const tall = { width: 240, height: 500 };
  expect(clampToWindow({ ...tall, x: 300, y: 350 }, view)).toEqual({
    left: 300,
    top: 192,
    maxHeight: null,
  });
});

it('scrolls a menu taller than the window', () => {
  const tall = { width: 240, height: 900 };
  expect(clampToWindow({ ...tall, x: 300, y: 100 }, view)).toEqual({
    left: 300,
    top: 8,
    maxHeight: 684,
  });
});

it('never puts a menu over the traffic lights', () => {
  const placed = clampToWindow({ ...menu, x: 20, y: 10 }, view);
  expect(placed).toEqual({ left: 20, top: 48, maxHeight: null });
  expect(clampToWindow({ ...menu, x: 84, y: 10 }, view).top).toBe(10);
  expect(clampToWindow({ ...menu, x: 20, y: 48 }, view).top).toBe(48);
});
