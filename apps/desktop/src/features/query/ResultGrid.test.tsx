// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridHandle, GridProps } from '@openrows/grid';
import type { WindowRequest } from '@openrows/ipc';
import { displayWindow } from '../../results/displayWindow';
import { updateTableSettings } from '../settings/tableSettings';
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
  vi.mocked(mountGrid).mockImplementation((_host, props) => {
    void props.source.window(
      {
        r0: 0,
        r1: props.source.rows - 1,
        c0: 0,
        c1: props.source.columns.length - 1,
      },
      0,
    );
    return {
      destroy: vi.fn(),
      viewport: { x: 0, y: 0, width: 800, height: 400 },
      scrollTo: vi.fn(),
      relayout,
      fitWidth: () => 140,
    } as unknown as GridHandle;
  });
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
    expect(mountGrid).toHaveBeenCalled();
  });
  return new Proxy({} as GridProps, {
    get: (_target, property): unknown =>
      Reflect.get(
        vi.mocked(mountGrid).mock.calls.at(-1)?.[1] as GridProps,
        property,
      ) as unknown,
  });
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
  await waitFor(() => {
    expect(grid.source.rows).toBe(3);
  });
  act(() => {
    grid.onHeader?.(1);
  });
  expect(screen.getByRole('status')).toHaveTextContent(/every row has arrived/);
  view.rerender(
    <ThemeProvider>
      <ResultGrid result={result()} engine="postgres" onCell={vi.fn()} />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(grid.source.cell(0, 1)?.text).toBe('1');
  });
  expect(grid.source.columns[1]).toMatchObject({
    title: 'id',
    sort: 'ascending',
  });
  expect(grid.rowHeaderColumn).toBe(0);
  expect(grid.source.cell(0, 0)?.text).toBe('1');
  expect(relayout).toHaveBeenCalled();
  const user = userEvent.setup();
  await user.type(screen.getByRole('textbox', { name: 'Filter rows' }), 'an');
  await waitFor(() => {
    expect(screen.getByRole('status')).toHaveTextContent('1 of 3 rows');
  });
  expect(grid.source.rows).toBe(1);
  expect(grid.source.cell(0, 2)?.text).toBe('banana');
});

it('reports the inspector cell in result coordinates and copies from the menu', async () => {
  const onCell = vi.fn();
  show(result(), onCell);
  const grid = await mounted();
  const ranges = [{ r0: 0, r1: 1, c0: 1, c1: 2 }];
  act(() => {
    grid.onSelection({ row: 1, column: 2, ranges });
  });
  expect(onCell).toHaveBeenLastCalledWith(
    expect.objectContaining({
      row: 1,
      index: 1,
      column: 'fruit',
      type: 'text',
    }),
  );
  const user = userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  act(() => {
    grid.onContextMenu?.({ row: 1, column: 2, kind: 'cell', x: 10, y: 10 });
  });
  expect(screen.getByRole('menu', { name: 'Table actions' })).toBeVisible();
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
      grid.onContextMenu?.({ row: 0, column: 2, kind: 'column', x: 5, y: 5 });
    });
  };
  open();
  await user.click(screen.getByRole('menuitem', { name: 'Pin column' }));
  expect(
    grid.source.columns.slice(1).map((column) => [column.title, column.pinned]),
  ).toEqual([
    ['fruit', true],
    ['id', false],
  ]);
  act(() => {
    grid.onContextMenu?.({ row: 0, column: 1, kind: 'column', x: 5, y: 5 });
  });
  await user.click(
    screen.getByRole('menuitem', { name: 'Size column to fit' }),
  );
  expect(grid.source.columns[1]?.width).toBe(140);
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toContain(
    '"fruit":140',
  );
  act(() => {
    grid.onColumnMove?.(2, 1);
  });
  // An unpinned column stays after the pinned ones.
  expect(grid.source.columns[1]?.title).toBe('fruit');
  open();
  await user.click(screen.getByRole('menuitem', { name: 'Reset columns' }));
  expect(grid.source.columns.slice(1).map((column) => column.title)).toEqual([
    'id',
    'fruit',
  ]);
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toBe('{}');
  act(() => {
    grid.onColumnResize?.(2, 215, true);
  });
  expect(grid.source.columns[2]?.width).toBe(215);
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toContain(
    '"fruit":215',
  );
});

it('applies striping settings live without remounting the result canvas', async () => {
  show(result());
  const grid = await mounted();
  expect(grid.striped).toBe(true);
  act(() => {
    updateTableSettings({ striped: false });
  });
  expect(grid.striped).toBe(false);
  expect(mountGrid).toHaveBeenCalledOnce();
});

it('opens a header quick filter and applies it to the matching result column', async () => {
  show(result());
  const grid = await mounted();
  act(() => {
    grid.onContextMenu?.({ row: 0, column: 2, kind: 'column', x: 5, y: 5 });
  });
  const user = userEvent.setup();
  await user.click(screen.getByRole('menuitem', { name: 'Filter column…' }));
  const value = screen.getByRole('textbox', { name: 'Value of fruit' });
  await user.type(value, 'ana{Enter}');
  await waitFor(() => {
    expect(grid.source.rows).toBe(1);
  });
  expect(grid.source.cell(0, 2)?.text).toBe('banana');
  await user.click(
    screen.getByRole('button', { name: 'Clear column filters (1)' }),
  );
  expect(grid.source.rows).toBe(3);
});

it('keeps repeated labels independent when resized, moved, pinned and hidden', async () => {
  show(
    result({
      columns: [
        { name: 'id', sourceType: 'int4' },
        { name: 'id', sourceType: 'text' },
      ],
    }),
  );
  const grid = await mounted();
  await waitFor(() => {
    expect(grid.source.cell(0, 2)?.text).toBe('cherry');
  });
  act(() => {
    grid.onColumnResize?.(1, 111, true);
  });
  act(() => {
    grid.onColumnResize?.(2, 222, true);
  });
  expect(localStorage.getItem('openrows.columnLayouts.v1')).toContain(
    '"id 2":222',
  );
  expect(grid.source.columns.slice(1).map((column) => column.width)).toEqual([
    111, 222,
  ]);
  expect(mountGrid).toHaveBeenCalledOnce();
  act(() => {
    grid.onColumnMove?.(2, 1);
  });
  await waitFor(() => {
    expect(grid.source.cell(0, 1)?.text).toBe('cherry');
  });
  expect(grid.source.columns.slice(1).map((column) => column.width)).toEqual([
    222, 111,
  ]);
  act(() => {
    grid.onContextMenu?.({ row: 0, column: 1, kind: 'column', x: 5, y: 5 });
  });
  const user = userEvent.setup();
  await user.click(screen.getByRole('menuitem', { name: 'Pin column' }));
  expect(grid.source.columns.slice(1).map((column) => column.pinned)).toEqual([
    true,
    false,
  ]);
  act(() => {
    grid.onContextMenu?.({ row: 0, column: 1, kind: 'column', x: 5, y: 5 });
  });
  await user.click(screen.getByRole('menuitem', { name: 'Hide column' }));
  await waitFor(() => {
    expect(grid.source.columns).toHaveLength(2);
  });
  expect(grid.source.cell(0, 1)?.text).toBe('3');
});
