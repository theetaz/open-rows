// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { TileSource } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import { ResultViewSource } from './ResultViewSource';
import { LocalRows } from './localRows';
import { resultColumns, columnKeys } from './resultColumns';

function load(request: WindowRequest): Promise<DisplayTile> {
  const cells = [];
  for (let row = 0; row < request.rowCount; row++)
    for (let column = 0; column < request.colCount; column++)
      cells.push({
        text: `${String(request.rowStart + row)}:${String(request.colStart + column)}`,
        state: 'normal' as const,
      });
  return Promise.resolve({ cells, columns: request.colCount, convertMs: 0 });
}

const columns = Array.from({ length: 30 }, (_, index) => ({
  id: String(index),
  title: `c${String(index)}`,
  width: 80,
  pinned: false,
}));
const result = {
  id: 'r',
  statement: 0,
  rows: 3,
  done: true,
  truncated: false,
  columns: columns.map((column) => ({
    name: column.title,
    sourceType: 'text',
  })),
};

it('maps arranged view columns to result columns and fetches their span', async () => {
  const spy = vi.fn(load);
  const view = new ResultViewSource(new TileSource('r', 3, columns, spy));
  // Pin result column 25 first, then the rest in result order.
  const arranged = resultColumns(
    result.columns,
    { widths: { c3: 200 }, order: [], pinned: ['c25'] },
    { column: 3, descending: true },
  );
  view.arrange(arranged.columns, arranged.order);
  expect(view.columns[0]).toMatchObject({ title: 'c25', pinned: true });
  expect(view.columns[4]).toMatchObject({
    title: 'c3',
    sort: 'descending',
    width: 200,
  });
  expect(view.columnOf(0)).toBe(25);
  expect(view.columnOf(1)).toBe(0);
  await view.window({ r0: 0, r1: 2, c0: 1, c1: 2 }, 0);
  expect(spy.mock.calls.map(([request]) => request.colStart)).toEqual([0, 20]);
  expect(view.cell(1, 0)?.text).toBe('1:25');
  expect(view.cell(2, 2)?.text).toBe('2:1');
});

it('shows sorted or filtered rows from loaded rows and grows while streaming', async () => {
  const base = new TileSource('r', 3, columns, load);
  const view = new ResultViewSource(base);
  const listener = vi.fn();
  view.subscribe(listener);
  const local = await LocalRows.load(result, load);
  view.show({ local, rows: Int32Array.from([2, 0]) });
  expect(view.rows).toBe(2);
  expect(view.rowOf(0)).toBe(2);
  expect(view.cell(0, 1)?.text).toBe('2:1');
  await expect(
    view.window({ r0: 0, r1: 1, c0: 0, c1: 1 }, 0),
  ).resolves.toBeUndefined();
  view.show(null);
  expect(view.rows).toBe(3);
  view.grow(10);
  expect(view.rows).toBe(10);
  expect(listener).toHaveBeenCalledTimes(3);
});

it('numbers repeated column names so layouts stay distinct', () => {
  expect(
    columnKeys([
      { name: 'id', sourceType: null },
      { name: 'id', sourceType: null },
      { name: 'x', sourceType: null },
    ]),
  ).toEqual(['id', 'id 2', 'x']);
});
