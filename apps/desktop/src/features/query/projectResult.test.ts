// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { emptyLayout } from '../../results/columnLayout';
import { displayWindow } from '../../results/displayWindow';
import { readSelection } from '../../results/copy/read';
import { readRows } from '../table/row';
import { LocalRows } from './localRows';
import { projectResult } from './projectResult';
import type { ResultMeta } from './run';
import { columnKeys } from './resultColumns';
vi.mock('../../results/displayWindow', () => ({ displayWindow: vi.fn() }));
const result: ResultMeta = {
  id: 'source',
  statement: 0,
  rows: 3,
  done: true,
  truncated: false,
  columns: ['id', 'value', 'key'].map((name) => ({ name, sourceType: 'text' })),
};
it('shares one safe coordinate mapping for filtered/reordered copies and hidden-key snapshots', async () => {
  vi.mocked(displayWindow).mockImplementation((request) =>
    Promise.resolve({
      columns: request.colCount,
      convertMs: 0,
      cells: [
        ['1', 'one', 'A'],
        ['2', 'two', 'B'],
        ['3', 'three', 'C'],
      ]
        .slice(request.rowStart, request.rowStart + request.rowCount)
        .flatMap((row) =>
          row
            .slice(request.colStart, request.colStart + request.colCount)
            .map((text) => ({ text, state: 'normal' as const })),
        ),
    }),
  );
  const local = await LocalRows.load(result, displayWindow);
  const projected = projectResult({
    result,
    layout: { ...emptyLayout, order: ['value', 'id', 'key'] },
    hidden: ['id', 'key'],
    shown: { local, rows: Int32Array.of(2, 0) },
  });
  const rows = await readRows(projected, [0, 1]);
  expect(rows.get(0)).toEqual([
    { name: 'value', index: 1, type: 'text', text: 'three', state: 'normal' },
    {
      name: 'id',
      index: 0,
      type: 'text',
      text: '3',
      state: 'normal',
      hidden: true,
    },
    {
      name: 'key',
      index: 2,
      type: 'text',
      text: 'C',
      state: 'normal',
      hidden: true,
    },
  ]);
  const copied = await readSelection(
    { result: projected, ranges: [{ r0: 0, r1: 1, c0: 0, c1: 0 }] },
    displayWindow,
  );
  expect(copied.rows).toEqual([['three'], ['one']]);
  expect(projected.projection?.rowOf(0)).toBe(2);
});

it('disambiguates repeated labels without colliding with an existing numbered label', () => {
  const columns = ['id', 'id', 'id', 'id 2'].map((name) => ({
    name,
    sourceType: null,
  }));
  expect(columnKeys(columns)).toEqual(['id', 'id 3', 'id 4', 'id 2']);
  const projected = projectResult({
    result: { ...result, columns },
    layout: { ...emptyLayout, order: ['id 4', 'id 2', 'id', 'id 3'] },
    hidden: ['id 3'],
    shown: null,
  });
  expect(projected.columnKeys).toEqual(['id 4', 'id 2', 'id', 'id 3']);
  expect(projected.columns.map((column) => column.name)).toEqual([
    'id',
    'id 2',
    'id',
  ]);
  expect(projected.projection?.columnOf(0)).toBe(2);
  expect(projected.projection?.columnOf(3)).toBe(1);
});
