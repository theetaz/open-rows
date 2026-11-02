// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { keyMove, leaveGrid } from './keyboard';
import { props } from './testFixture';
import type { SelectionModel } from './model';

afterEach(() => {
  vi.restoreAllMocks();
  document.body.replaceChildren();
});
const at = (row: number, column: number): SelectionModel => ({
  row,
  column,
  ranges: [{ r0: row, r1: row, c0: column, c1: column }],
});
const key = (name: string, extra: KeyboardEventInit = {}): KeyboardEvent =>
  new KeyboardEvent('keydown', { key: name, ...extra });

it('pages by the viewport, clamps at the ends and extends with Shift', () => {
  const input = props();
  const down = keyMove(at(10, 3), key('PageDown'), input, 30);
  expect(down).toMatchObject({
    selection: { row: 40, column: 3 },
    scrollRows: 30,
  });
  const up = keyMove(at(10, 3), key('PageUp'), input, 30);
  expect(up).toMatchObject({
    selection: { row: 0, column: 3 },
    scrollRows: -10,
  });
  const last = keyMove(at(999_990, 3), key('PageDown'), input, 30);
  expect(last).toMatchObject({ selection: { row: 999_999 }, scrollRows: 9 });
  const extended = keyMove(
    { ...at(10, 3), anchor: { row: 10, column: 3 } },
    key('PageDown', { shiftKey: true }),
    input,
    30,
  );
  expect(extended?.selection.ranges).toEqual([
    { r0: 10, r1: 40, c0: 3, c1: 3 },
  ]);
  expect(
    keyMove(at(10, 3), key('PageDown', { ctrlKey: true }), input, 30),
  ).toBeUndefined();
});

it('goes to the row ends with Home/End and to the first or last cell with Cmd/Ctrl', () => {
  const input = props();
  input.rowHeaderColumn = 0;
  expect(keyMove(at(7, 40), key('Home'), input, 30)?.selection).toMatchObject({
    row: 7,
    column: 1,
  });
  expect(keyMove(at(7, 40), key('End'), input, 30)?.selection).toMatchObject({
    row: 7,
    column: 99,
  });
  expect(
    keyMove(at(7, 40), key('Home', { metaKey: true }), input, 30)?.selection,
  ).toMatchObject({ row: 0, column: 1 });
  expect(
    keyMove(at(7, 40), key('End', { ctrlKey: true }), input, 30)?.selection,
  ).toMatchObject({ row: 999_999, column: 99 });
  const range = keyMove(at(7, 40), key('End', { shiftKey: true }), input, 30);
  expect(range?.selection.ranges).toEqual([{ r0: 7, r1: 7, c0: 40, c1: 99 }]);
  expect(range?.scrollRows).toBe(0);
});

it('tabs through cells in reading order and wraps between rows', () => {
  const input = props();
  input.rowHeaderColumn = 0;
  expect(keyMove(at(4, 5), key('Tab'), input, 30)?.selection).toMatchObject({
    row: 4,
    column: 6,
  });
  expect(keyMove(at(4, 99), key('Tab'), input, 30)?.selection).toEqual({
    ...at(5, 1),
    anchor: { row: 5, column: 1 },
  });
  expect(
    keyMove(at(4, 1), key('Tab', { shiftKey: true }), input, 30)?.selection,
  ).toMatchObject({ row: 3, column: 99 });
  expect(
    keyMove(at(4, 1), key('Tab', { altKey: true }), input, 30),
  ).toBeUndefined();
});

it('leaves Tab to the browser at the first and last cells and in an empty grid', () => {
  const input = props();
  expect(keyMove(at(999_999, 99), key('Tab'), input, 30)).toBeUndefined();
  expect(
    keyMove(at(0, 0), key('Tab', { shiftKey: true }), input, 30),
  ).toBeUndefined();
  Object.defineProperty(input.source, 'rows', { value: 0 });
  expect(keyMove(at(0, 0), key('Tab'), input, 30)).toBeUndefined();
});

it('moves focus to the nearest rendered tabbable element outside the grid', () => {
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(
    function (this: HTMLElement) {
      return (this.hidden ? [] : [new DOMRect()]) as unknown as DOMRectList;
    },
  );
  const before = document.createElement('button'),
    grid = document.createElement('div'),
    inside = document.createElement('button'),
    hidden = document.createElement('button'),
    after = document.createElement('input');
  hidden.hidden = true;
  grid.append(inside);
  document.body.append(before, grid, hidden, after);
  inside.focus();
  expect(leaveGrid(grid, false)).toBe(true);
  expect(document.activeElement).toBe(after);
  expect(leaveGrid(grid, true)).toBe(true);
  expect(document.activeElement).toBe(before);
  after.remove();
  expect(leaveGrid(grid, false)).toBe(false);
});
