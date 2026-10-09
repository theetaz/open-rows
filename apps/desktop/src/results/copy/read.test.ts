// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import { COPY_ROW_LIMIT, CopyError, readSelection } from './read';

const result = {
  id: 'r',
  statement: 0,
  rows: 10_000,
  done: true,
  truncated: false,
  columns: [
    { name: 'id', sourceType: 'int8' },
    { name: 'name', sourceType: 'text' },
    { name: 'ok', sourceType: 'bool' },
  ],
};

/** Cell text `r<row>c<column>`; column 2 of row 0 is NULL, of row 1 empty. */
function load(request: WindowRequest): Promise<DisplayTile> {
  const cells: DisplayTile['cells'] = [];
  for (let row = 0; row < request.rowCount; row++)
    for (let column = 0; column < request.colCount; column++) {
      const r = request.rowStart + row,
        c = request.colStart + column;
      if (c === 2 && r === 0) cells.push({ text: 'NULL', state: 'null' });
      else if (c === 2 && r === 1)
        cells.push({ text: 'EMPTY', state: 'empty' });
      else cells.push({ text: `r${String(r)}c${String(c)}`, state: 'normal' });
    }
  return Promise.resolve({ cells, columns: request.colCount, convertMs: 0 });
}

it('reads the selected rows and columns with NULL and empty values', async () => {
  const table = await readSelection(
    {
      result,
      ranges: [
        { r0: 0, r1: 1, c0: 1, c1: 2 },
        { r0: 1, r1: 1, c0: 0, c1: 0 },
      ],
    },
    load,
  );
  expect(table.columns).toEqual([
    { name: 'id', kind: 'number' },
    { name: 'name', kind: 'text' },
    { name: 'ok', kind: 'boolean' },
  ]);
  expect(table.rows).toEqual([
    ['r0c0', 'r0c1', null],
    ['r1c0', 'r1c1', ''],
  ]);
});

it('maps view rows and columns and splits scattered rows into small windows', async () => {
  const spy = vi.fn(load);
  const table = await readSelection(
    {
      result,
      ranges: [{ r0: 0, r1: 1, c0: 0, c1: 1 }],
      rowOf: (row) => (row === 0 ? 9000 : 3),
      columnOf: (column) => (column === 0 ? 2 : 1),
    },
    spy,
  );
  expect(table.columns.map((column) => column.name)).toEqual(['ok', 'name']);
  expect(table.rows).toEqual([
    ['r9000c2', 'r9000c1'],
    ['r3c2', 'r3c1'],
  ]);
  expect(spy.mock.calls.map(([request]) => request.rowStart)).toEqual([
    3, 9000,
  ]);
  expect(await readSelection({ result, ranges: [] }, load)).toEqual({
    columns: [],
    rows: [],
  });
});

it('refuses selections beyond the copy limit', async () => {
  await expect(
    readSelection(
      { result, ranges: [{ r0: 0, r1: COPY_ROW_LIMIT, c0: 0, c1: 0 }] },
      load,
    ),
  ).rejects.toBeInstanceOf(CopyError);
});
