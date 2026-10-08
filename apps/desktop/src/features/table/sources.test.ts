// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { TileSource } from '@openrows/grid';
import { NumberedSource } from './NumberedSource';
import { EditedSource } from './EditedSource';
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
