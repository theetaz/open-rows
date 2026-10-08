// SPDX-License-Identifier: Apache-2.0
import { NULL_VALUE, textValue } from './cellValue';
import { expect, it, vi } from 'vitest';
import { TileSource } from '@openrows/grid';
import { NumberedSource } from './NumberedSource';
import { EditedSource, stagedCell } from './EditedSource';
const columns = ['id', 'name'].map((name) => ({
  id: name,
  title: name,
  width: 100,
  pinned: false,
}));
const request = { r0: 0, r1: 30, c0: 0, c1: 2 };
function source() {
  const load = vi.fn(
    (request: {
      rowStart: number;
      rowCount: number;
      colCount: number;
      colStart: number;
    }) =>
      Promise.resolve({
        rows: request.rowCount,
        columns: request.colCount,
        convertMs: 0,
        cells: Array.from(
          { length: request.rowCount * request.colCount },
          (_, index) => ({
            text:
              (index % request.colCount) + request.colStart === 0
                ? String(
                    request.rowStart + Math.floor(index / request.colCount) + 1,
                  )
                : 'old',
            state: 'normal' as const,
          }),
        ),
      }),
  );
  return { source: new TileSource('result', 300, columns, load), load };
}
it('pins page-aware numbers without shifting IPC requests, including rows scrolled within a tile', async () => {
  const base = source();
  const numbered = new NumberedSource(base.source, 300);
  await numbered.window(request, 0);
  expect(numbered.columns[0]?.pinned).toBe(true);
  expect(numbered.cell(0, 0)?.text).toBe('301');
  expect(numbered.cell(100, 0)?.text).toBe('401');
  expect(numbered.cell(0, 1)?.text).toBe('1');
  expect(base.load.mock.calls[0]?.[0].colStart).toBe(0);
});
it('matches staged edits by stable keys after sorting and prepares highlights beyond the first viewport', async () => {
  const base = source();
  const edited = new EditedSource(
    base.source,
    [
      {
        id: 'key',
        number: 1,
        kind: 'update',
        original: { id: '101', name: 'old' },
        values: { name: { kind: 'text', value: 'updated' } },
      },
    ],
    { keys: ['id'], names: ['id', 'name'] },
  );
  await edited.window({ ...request, c1: 1 }, 0);
  expect(edited.cell(0, 1)).toEqual({ text: 'old', state: 'normal' });
  expect(edited.cell(100, 1)).toEqual({ text: 'updated', state: 'changed' });
});
it('swaps the staged overlay in place and notifies the grid to repaint', async () => {
  const base = source(),
    keys = ['id'];
  const edited = new EditedSource(base.source, [], {
    keys,
    names: ['id', 'name'],
  });
  const repaint = vi.fn();
  edited.subscribe(repaint);
  await edited.window({ ...request, c1: 1 }, 0);
  repaint.mockClear();
  const changes = [
    {
      id: 'key',
      number: 19,
      kind: 'update' as const,
      original: { id: '19', name: 'old' },
      values: { name: { kind: 'text' as const, value: 'staged' } },
    },
  ];
  edited.stage(changes, keys);
  expect(edited.cell(18, 1)).toEqual({ text: 'staged', state: 'changed' });
  edited.stage(changes, keys);
  edited.stage([], keys);
  expect(edited.cell(18, 1)).toEqual({ text: 'old', state: 'normal' });
  expect(repaint).toHaveBeenCalledTimes(2);
});
it('paints staged NULL, EMPTY and an unset insert column as distinct muted markers', () => {
  expect(stagedCell(NULL_VALUE, 'changed')).toEqual({
    text: 'NULL',
    state: 'changed',
    marker: 'null',
  });
  expect(stagedCell(textValue(''), 'changed')).toEqual({
    text: 'EMPTY',
    state: 'changed',
    marker: 'empty',
  });
  expect(stagedCell(undefined, 'inserted')).toEqual({
    text: 'DEFAULT',
    state: 'inserted',
    marker: 'default',
  });
  expect(stagedCell(textValue('NULL'), 'changed')).toEqual({
    text: 'NULL',
    state: 'changed',
  });
});
it('appends staged inserts after the page without fetching them, and numbers them as new', async () => {
  const base = source(),
    keys = ['id'];
  const edited = new EditedSource(base.source, [], {
    keys,
    names: ['id', 'name'],
  });
  const insert = {
    id: 'new',
    number: 0,
    kind: 'insert' as const,
    original: {},
    values: { name: textValue('typed') },
  };
  edited.stage([insert], keys);
  expect(edited.rows).toBe(301);
  expect(edited.cell(300, 0)).toMatchObject({ marker: 'default' });
  expect(edited.cell(300, 1)).toEqual({ text: 'typed', state: 'inserted' });
  await edited.window({ r0: 290, r1: 300, c0: 0, c1: 1 }, 1);
  const rows = base.load.mock.calls.map(
    ([call]) => call.rowStart + call.rowCount,
  );
  expect(Math.max(...rows)).toBeLessThanOrEqual(300);
  const numbered = new NumberedSource(edited, 0, 300);
  expect(numbered.cell(300, 0)).toMatchObject({ text: '+', state: 'inserted' });
  expect(numbered.cell(299, 0)?.text).toBe('300');
  edited.stage([], keys);
  expect(edited.rows).toBe(300);
});
it('reports changed column kinds so the grid re-measures text', () => {
  const base = source();
  const edited = new EditedSource(base.source, [], {
    keys: [],
    names: ['id', 'name'],
  });
  expect(edited.describe([undefined, 'enum'])).toBe(true);
  expect(edited.columns[1]?.kind).toBe('enum');
  expect(edited.describe([undefined, 'enum'])).toBe(false);
  expect(edited.describe([])).toBe(true);
  expect(edited.columns[1]?.kind).toBeUndefined();
});
