// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridHandle, GridProps } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import { displayWindow } from '../../results/displayWindow';
import { ResultGrid } from './ResultGrid';
import type { ResultMeta } from './run';

vi.mock('@openrows/grid/canvas', () => ({ mountGrid: vi.fn() }));
vi.mock('../../results/displayWindow', () => ({ displayWindow: vi.fn() }));

const values = [
  ['3', 'cherry'],
  ['1', 'apple'],
  ['2', 'banana'],
];
const relayout = vi.fn();

beforeEach(() => {
  localStorage.clear();
  relayout.mockClear();
  vi.mocked(mountGrid).mockReset();
  vi.mocked(mountGrid).mockImplementation(
    () =>
      ({
        destroy: vi.fn(),
        relayout,
        fitWidth: () => 140,
      }) as unknown as GridHandle,
  );
  vi.mocked(displayWindow).mockImplementation((request: WindowRequest) => {
    const cells = values
      .slice(request.rowStart, request.rowStart + request.rowCount)
      .flatMap((row) =>
        row
          .slice(request.colStart, request.colStart + request.colCount)
          .map((text) => ({ text, state: 'normal' as const })),
      );
    return Promise.resolve({ cells, columns: request.colCount, convertMs: 0 });
  });
});

const result = (change: Partial<ResultMeta> = {}): ResultMeta => ({
  id: 'r1',
  statement: 0,
  rows: 3,
  done: true,
  truncated: false,
  columns: [
    { name: 'id', sourceType: 'int4' },
    { name: 'fruit', sourceType: 'text' },
  ],
  ...change,
});

function show(meta: ResultMeta, onCell = vi.fn()): ReturnType<typeof render> {
  return render(
    <ThemeProvider>
      <ResultGrid result={meta} engine="postgres" onCell={onCell} />
    </ThemeProvider>,
  );
}

async function mounted(): Promise<GridProps> {
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalledOnce();
  });
  return vi.mocked(mountGrid).mock.calls[0]?.[1] as GridProps;
}

it('grows while rows stream, then sorts by header and filters loaded rows', async () => {
  const view = show(result({ rows: 1, done: false }));
  const grid = await mounted();
  expect(grid.source.rows).toBe(1);
  view.rerender(
    <ThemeProvider>
      <ResultGrid
        result={result({ done: false })}
        engine="postgres"
        onCell={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(grid.source.rows).toBe(3);
  act(() => {
    grid.onHeader?.(0);
  });
  expect(screen.getByRole('status')).toHaveTextContent(/every row has arrived/);
  view.rerender(
    <ThemeProvider>
      <ResultGrid result={result()} engine="postgres" onCell={vi.fn()} />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(grid.source.cell(0, 0)?.text).toBe('1');
  });
  expect(grid.source.columns[0]?.title).toBe('id ▲');
  expect(mountGrid).toHaveBeenCalledOnce();
  expect(relayout).toHaveBeenCalled();
  const user = userEvent.setup();
  await user.type(screen.getByRole('textbox', { name: 'Filter rows' }), 'an');
  await waitFor(() => {
    expect(screen.getByRole('status')).toHaveTextContent('1 of 3 rows');
  });
  expect(grid.source.rows).toBe(1);
  expect(grid.source.cell(0, 1)?.text).toBe('banana');
});

it('reports the inspector cell in result coordinates and copies from the menu', async () => {
  const onCell = vi.fn();
  show(result(), onCell);
  const grid = await mounted();
  const ranges = [{ r0: 0, r1: 1, c0: 0, c1: 1 }];
  act(() => {
    grid.onSelection({ row: 1, column: 1, ranges });
  });
  expect(onCell).toHaveBeenLastCalledWith(
    expect.objectContaining({
      row: 1,
      index: 1,
      column: 'fruit',
      type: 'text',
    }),
  );
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  act(() => {
    grid.onContextMenu?.({ row: 1, column: 1, kind: 'cell', x: 10, y: 10 });
  });
  const user = userEvent.setup();
  expect(screen.getByRole('menu', { name: 'Result actions' })).toBeVisible();
  await user.click(screen.getByRole('menuitem', { name: 'Copy as' }));
  await user.click(screen.getByRole('menuitem', { name: 'CSV' }));
  await waitFor(() => {
    expect(write).toHaveBeenCalledWith('id,fruit\n3,cherry\n1,apple');
  });
  expect(screen.queryByRole('menu')).toBeNull();
});

it('pins, sizes and resets columns from the header menu, persisting the layout', async () => {
  show(result());
  const grid = await mounted();
  const user = userEvent.setup();
  const open = (): void => {
    act(() => {
      grid.onContextMenu?.({ row: 0, column: 1, kind: 'column', x: 5, y: 5 });
    });
  };
  open();
  await user.click(screen.getByRole('menuitem', { name: 'Pin column' }));
  expect(
    grid.source.columns.map((column) => [column.title, column.pinned]),
  ).toEqual([
    ['fruit', true],
    ['id', false],
  ]);
  act(() => {
    grid.onContextMenu?.({ row: 0, column: 0, kind: 'column', x: 5, y: 5 });
  });
  await user.click(
    screen.getByRole('menuitem', { name: 'Size column to fit' }),
  );
  expect(grid.source.columns[0]?.width).toBe(140);
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toContain(
    '"fruit":140',
  );
  act(() => {
    grid.onColumnMove?.(1, 0);
  });
  // An unpinned column stays after the pinned ones.
  expect(grid.source.columns[0]?.title).toBe('fruit');
  open();
  await user.click(screen.getByRole('menuitem', { name: 'Reset columns' }));
  expect(grid.source.columns.map((column) => column.title)).toEqual([
    'id',
    'fruit',
  ]);
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toBe('{}');
});
