// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { clampToWindow } from './placement';

const view = { width: 1000, height: 700 };
const menu = { width: 240, height: 200 };

it('keeps a menu 6 px inside every window edge', () => {
  expect(clampToWindow({ ...menu, x: 900, y: 100 }, view)).toEqual({
    left: 754,
    top: 100,
    maxHeight: null,
  });
  expect(clampToWindow({ ...menu, x: -20, y: 100 }, view).left).toBe(6);
});

it('opens upward when there is no room below', () => {
  const placed = clampToWindow({ ...menu, x: 300, y: 600, flip: 590 }, view);
  expect(placed).toEqual({ left: 300, top: 390, maxHeight: null });
});

it('opens an above menu downward when there is no room above', () => {
  const placed = clampToWindow({ ...menu, x: 300, y: 120, above: true }, view);
  expect(placed.top).toBe(120);
});

it('pins a menu that fits neither side 6 px from the bottom', () => {
  const tall = { width: 240, height: 500 };
  expect(clampToWindow({ ...tall, x: 300, y: 350 }, view)).toEqual({
    left: 300,
    top: 194,
    maxHeight: null,
  });
});

it('scrolls a menu taller than the window', () => {
  const tall = { width: 240, height: 900 };
  expect(clampToWindow({ ...tall, x: 300, y: 100 }, view)).toEqual({
    left: 300,
    top: 6,
    maxHeight: 688,
  });
});

it('never puts a menu over the traffic lights', () => {
  const placed = clampToWindow({ ...menu, x: 20, y: 10 }, view);
  expect(placed).toEqual({ left: 20, top: 48, maxHeight: null });
  expect(clampToWindow({ ...menu, x: 84, y: 10 }, view).top).toBe(10);
  expect(clampToWindow({ ...menu, x: 20, y: 48 }, view).top).toBe(48);
});

it('flips a pointer menu to the left of the pointer near the right edge', () => {
  const placed = clampToWindow({ ...menu, x: 901, y: 100, flipX: 900 }, view);
  expect(placed.left).toBe(660);
  expect(
    clampToWindow({ ...menu, x: 101, y: 100, flipX: 100 }, view).left,
  ).toBe(101);
});
