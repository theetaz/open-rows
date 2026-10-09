// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { GridLayout } from './layout';
import {
  dragWidth,
  dropIndex,
  dropLine,
  fitWidth,
  MAX_AUTO_WIDTH,
  MIN_COLUMN_WIDTH,
  resizeEdge,
} from './columns';

const layout = new GridLayout(
  Array.from({ length: 6 }, (_, index) => ({
    id: String(index),
    title: String(index),
    width: 120,
    pinned: index < 2,
  })),
  26,
);
const view = (
  x: number,
): { x: number; y: number; width: number; height: number } => ({
  x,
  y: 0,
  width: 600,
  height: 300,
});

it('finds header edges of pinned and scrolled columns within the slop', () => {
  expect(resizeEdge(layout, view(0), 120, 10)).toBe(0);
  expect(resizeEdge(layout, view(0), 243, 10)).toBe(1);
  expect(resizeEdge(layout, view(0), 357, 10)).toBe(2);
  expect(resizeEdge(layout, view(100), 260, 10)).toBe(2);
  expect(resizeEdge(layout, view(0), 60, 10)).toBeUndefined();
  // Below the header row nothing resizes.
  expect(resizeEdge(layout, view(0), 120, 40)).toBeUndefined();
  // Column 2's edge has scrolled behind the pinned prefix.
  expect(resizeEdge(layout, view(200), 160, 10)).toBeUndefined();
});

it('computes the index a dragged header lands on and where its marker goes', () => {
  // Left half of column 2: column 0 lands just before it.
  expect(dropIndex(layout, view(0), 300, 0)).toBe(1);
  expect(dropIndex(layout, view(0), 310, 0)).toBe(2);
  // Dragging right-to-left keeps the slot index.
  expect(dropIndex(layout, view(0), 10, 4)).toBe(0);
  expect(dropIndex(layout, view(120), 300, 5)).toBe(3);
  expect(dropLine(layout, view(0), 1, 0)).toBe(240);
  expect(dropLine(layout, view(100), 3, 0)).toBe(380);
  expect(dropLine(layout, view(0), 0, 4)).toBe(0);
});

it('clamps fitted and dragged widths', () => {
  expect(fitWidth([10, 70, 30])).toBe(86);
  expect(fitWidth([70], 16)).toBe(102);
  expect(fitWidth([])).toBe(MIN_COLUMN_WIDTH);
  expect(fitWidth([5000])).toBe(MAX_AUTO_WIDTH);
  expect(dragWidth(120, 35.4)).toBe(155);
  expect(dragWidth(120, -500)).toBe(MIN_COLUMN_WIDTH);
});
