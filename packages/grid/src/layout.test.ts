// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { GridLayout, backingSize, snap } from './layout';
const columns = [80, 120, 60, 200].map((width, index) => ({
  id: String(index),
  title: String(index),
  width,
  pinned: index < 2,
}));
it('computes prefix offsets, visible windows and pinned hit tests', () => {
  const layout = new GridLayout(columns, 26),
    range = { r0: 0, r1: 0, c0: 0, c1: 0 };
  const view = { x: 100, y: 260, width: 300, height: 260 };
  expect(Array.from(layout.offsets)).toEqual([0, 80, 200, 260, 460]);
  expect(layout.pinnedWidth).toBe(200);
  layout.visible(range, view, 1_000_000);
  expect(range).toEqual({ r0: 9, r1: 19, c0: 2, c1: 3 });
  expect(layout.hit(view, 100, 39)).toEqual({ row: 10, column: 1 });
  expect(layout.hit(view, 230, 39)).toEqual({ row: 10, column: 3 });
  expect(layout.hit(view, 10, 12)).toBeUndefined();
});
it('clamps first/last rows, exact column boundaries, and backing pixels', () => {
  const layout = new GridLayout(columns, 26),
    range = { r0: 0, r1: 0, c0: 0, c1: 0 };
  layout.visible(range, { x: 0, y: 0, width: 300, height: 260 }, 3);
  expect(range.r0).toBe(0);
  expect(range.r1).toBe(2);
  expect(layout.columnAt(80)).toBe(1);
  expect(layout.columnAt(460)).toBe(3);
  for (const dpr of [1, 1.25, 2, 3]) {
    expect(backingSize(301, dpr)).toBe(Math.round(301 * dpr));
    expect(snap(10.3, dpr) * dpr).toBeCloseTo(Math.round(10.3 * dpr));
  }
});
it('rejects invalid widths and non-leading pins', () => {
  expect(
    () =>
      new GridLayout([{ id: 'a', title: 'a', pinned: false, width: -1 }], 26),
  ).toThrow('width');
  expect(
    () =>
      new GridLayout(
        [
          { id: 'a', title: 'a', pinned: false, width: 80 },
          { id: 'b', title: 'b', pinned: true, width: 120 },
        ],
        26,
      ),
  ).toThrow('lead');
});
it('hit-tests header cells for sorting, respecting pinned columns', () => {
  const layout = new GridLayout(columns, 26);
  const view = { x: 100, y: 260, width: 300, height: 260 };
  expect(layout.headerHit(view, 100, 12)).toBe(1);
  expect(layout.headerHit(view, 230, 12)).toBe(3);
  expect(layout.headerHit(view, 230, 30)).toBeUndefined();
  expect(layout.headerHit({ ...view, x: 0 }, 470, 4)).toBeUndefined();
  expect(new GridLayout([], 26).headerHit(view, 4, 4)).toBeUndefined();
});
