// SPDX-License-Identifier: Apache-2.0
// A keyless SQLite table's rowid: selected last, never shown, still the row identity.
import { beforeEach, expect, it, vi } from 'vitest';
import { TileSource } from '@openrows/grid';
import type { ColumnDto } from '@openrows/ipc';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from '../query/run';
import { EditedSource } from './EditedSource';
import { initialView, tableQuery } from './model';
import { hideRowId, holdsColumn, readRows, shownFields } from './row';
import { stagedEditFor, withStaged } from './selectedRow';

vi.mock('../../results/displayWindow', () => ({ displayWindow: vi.fn() }));

const ROWID = 24;
const names = [
  ...Array.from({ length: ROWID }, (_, index) => `c${String(index)}`),
  'rowid',
];
const gridColumns = names.map((name) => ({
  id: name,
  title: name,
  width: 80,
  pinned: false,
}));

/** Every shown cell reads `same` (duplicate rows); the rowid is `row + 1`. */
function tiles() {
  const load = vi.fn(
    (request: {
      rowStart: number;
      rowCount: number;
      colStart: number;
      colCount: number;
    }) =>
      Promise.resolve({
        rows: request.rowCount,
        columns: request.colCount,
        convertMs: 0,
        cells: Array.from(
          { length: request.rowCount * request.colCount },
          (_, index) => {
            const column = request.colStart + (index % request.colCount);
            const row = request.rowStart + Math.floor(index / request.colCount);
            const text = column === ROWID ? String(row + 1) : 'same';
            return { text, state: 'normal' as const };
          },
        ),
      }),
  );
  return { source: new TileSource('page', 10, gridColumns, load), load };
}

const update = {
  id: '{"rowid":"3"}',
  number: 3,
  kind: 'update' as const,
  original: { c0: 'same', rowid: '3' },
  values: { c0: { kind: 'text' as const, value: 'edited' } },
};

it('hides the rowid from the grid but fetches it to match identical rows', async () => {
  const base = tiles();
  const edited = new EditedSource(base.source, [update], {
    keys: ['rowid'],
    names,
    shown: ROWID,
  });
  expect(edited.columns.map((column) => column.id)).toEqual(
    names.slice(0, ROWID),
  );
  await edited.window({ r0: 0, r1: 9, c0: 0, c1: 3 }, 0);
  const starts = base.load.mock.calls.map(([request]) => request.colStart);
  expect(starts).toContain(20);
  expect(edited.cell(2, 0)).toEqual({ text: 'edited', state: 'changed' });
  expect(edited.cell(1, 0)).toEqual({ text: 'same', state: 'normal' });
  expect(edited.cell(3, 0)).toEqual({ text: 'same', state: 'normal' });
  expect(edited.cell(2, 1)).toEqual({ text: 'same', state: 'normal' });
});

it('stages inserts with the shown columns only', () => {
  const insert = {
    ...update,
    id: 'new',
    kind: 'insert' as const,
    original: {},
  };
  const edited = new EditedSource(tiles().source, [insert], {
    keys: ['rowid'],
    names,
    shown: ROWID,
  });
  expect(edited.rows).toBe(11);
  expect(edited.cell(10, 0)).toEqual({ text: 'edited', state: 'inserted' });
  expect(edited.cell(10, ROWID)).toBeUndefined();
});

const column = (name: string): ColumnDto => ({ name, sourceType: 'TEXT' });
const page: ResultMeta = {
  id: 'page',
  statement: 0,
  columns: [column('a'), column('b'), { name: 'rowid', sourceType: null }],
  rows: 2,
  done: true,
  truncated: false,
};

it('splits a trailing rowid off the page, stably', () => {
  const split = hideRowId(page, 'rowid');
  expect(split?.columns.map((item) => item.name)).toEqual(['a', 'b']);
  expect(split?.hidden?.map((item) => item.name)).toEqual(['rowid']);
  expect(hideRowId(page, 'rowid')).toBe(split);
  expect(hideRowId(page, null)).toBe(page);
  expect(hideRowId(page, '_rowid_')).toBe(page);
  expect(hideRowId(undefined, 'rowid')).toBeUndefined();
  expect(split && holdsColumn(split, 'rowid')).toBe(true);
  expect(split && holdsColumn(split, 'oid')).toBe(false);
});

beforeEach(() => {
  vi.mocked(displayWindow).mockResolvedValue({
    cells: ['x', 'y', '7', 'x', 'y', '8'].map((text) => ({
      text,
      state: 'normal' as const,
    })),
    columns: 3,
    convertMs: 0,
  });
});

it('reads the rowid for identity while the inspector and copies never see it', async () => {
  const split = hideRowId(page, 'rowid');
  if (!split) throw new Error('no page');
  const rows = await readRows(split, [0, 1]);
  expect(vi.mocked(displayWindow).mock.lastCall?.[0].colCount).toBe(3);
  const second = rows.get(1) ?? [];
  expect(second.at(-1)).toMatchObject({
    name: 'rowid',
    text: '8',
    hidden: true,
  });
  const staged = { ...update, original: { a: 'x', b: 'y', rowid: '8' } };
  expect(stagedEditFor(second, [staged], ['rowid'])).toBe(staged);
  expect(stagedEditFor(rows.get(0) ?? [], [staged], ['rowid'])).toBeUndefined();
  const inspector = withStaged(shownFields(second), staged, null);
  expect(inspector.map((field) => field.name)).toEqual(['a', 'b']);
});

it('requests the rowid only when the table has one', () => {
  const target = {
    profileId: 'p',
    database: 'main',
    schema: 'main',
    table: 't',
  };
  expect(tableQuery(target, initialView, ['a'], 'rowid').rowId).toBe('rowid');
  expect('rowId' in tableQuery(target, initialView, ['a'])).toBe(false);
  expect('rowId' in tableQuery(target, initialView, ['a'], null)).toBe(false);
});
