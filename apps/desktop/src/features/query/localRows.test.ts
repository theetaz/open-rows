// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import type { DisplayCell } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import {
  LOCAL_CELL_LIMIT,
  LocalRows,
  filterRows,
  localLimit,
  sortRows,
  viewRows,
} from './localRows';

const values: readonly (readonly (string | null)[])[] = [
  ['10', 'banana'],
  ['9', null],
  ['100', 'Apple'],
  [null, 'cherry'],
  ['9', 'apple pie'],
];
const result = {
  id: 'r',
  statement: 0,
  rows: values.length,
  done: true,
  truncated: false,
  columns: [
    { name: 'n', sourceType: 'int4' },
    { name: 'fruit', sourceType: 'text' },
  ],
};

function cell(value: string | null): DisplayCell {
  return value === null
    ? { text: 'NULL', state: 'null' }
    : { text: value, state: 'normal' };
}

function load(request: WindowRequest): Promise<DisplayTile> {
  const cells = values
    .slice(request.rowStart, request.rowStart + request.rowCount)
    .flatMap((row) =>
      row
        .slice(request.colStart, request.colStart + request.colCount)
        .map(cell),
    );
  return Promise.resolve({ cells, columns: request.colCount, convertMs: 0 });
}

it('explains when sort and filter cannot run locally', () => {
  expect(localLimit(result)).toBeNull();
  expect(localLimit({ ...result, done: false })).toMatch(
    /every row has arrived/,
  );
  expect(localLimit({ ...result, rows: LOCAL_CELL_LIMIT })).toMatch(
    /1,000,000 cells/,
  );
});

it('loads every row once in chunks and looks cells up by row and column', async () => {
  const spy = vi.fn(load);
  const local = await LocalRows.load({ ...result, rows: 5000 }, spy);
  expect(
    spy.mock.calls.map(([request]) => [request.rowStart, request.rowCount]),
  ).toEqual([
    [0, 4096],
    [4096, 904],
  ]);
  expect(local.cell(2, 1)).toEqual({ text: 'Apple', state: 'normal' });
});

it('filters case-insensitively, never matching NULL markers', async () => {
  const local = await LocalRows.load(result, load);
  expect(filterRows(local, ' APPLE ')).toEqual([2, 4]);
  expect(filterRows(local, 'null')).toEqual([]);
  expect(filterRows(local, '9')).toEqual([1, 4]);
});

it('sorts numerically or by text, stable, with NULLs last both ways', async () => {
  const local = await LocalRows.load(result, load);
  const all = [0, 1, 2, 3, 4];
  expect(sortRows(local, all, { column: 0, descending: false }, true)).toEqual([
    1, 4, 0, 2, 3,
  ]);
  expect(sortRows(local, all, { column: 0, descending: true }, true)).toEqual([
    2, 0, 1, 4, 3,
  ]);
  // Code-point order: capitals first, as the data was stored.
  expect(sortRows(local, all, { column: 1, descending: false }, false)).toEqual(
    [2, 4, 0, 3, 1],
  );
  expect(
    viewRows(local, { needle: '', columns: [] }, null, () => false),
  ).toBeNull();
  expect(
    Array.from(
      viewRows(
        local,
        { needle: 'a', columns: [] },
        { column: 0, descending: true },
        (column) => column === 0,
      ) ?? [],
    ),
  ).toEqual([2, 0, 4]);
});

it('narrows rows by header quick filters on top of the search text', async () => {
  const local = await LocalRows.load(result, load);
  const view = (filter: Parameters<typeof viewRows>[1]): number[] =>
    Array.from(viewRows(local, filter, null, () => false) ?? []);
  const fruit = { column: 1, value: '' };
  expect(
    view({
      needle: '',
      columns: [{ ...fruit, operator: 'contains', value: 'APP' }],
    }),
  ).toEqual([2, 4]);
  expect(
    view({ needle: '', columns: [{ ...fruit, operator: 'isNull' }] }),
  ).toEqual([1]);
  expect(
    view({
      needle: '9',
      columns: [{ column: 1, operator: 'notEqual', value: 'banana' }],
    }),
  ).toEqual([4]);
  expect(
    view({
      needle: '',
      columns: [{ column: 0, operator: 'equal', value: '9' }],
    }),
  ).toEqual([1, 4]);
});
