// SPDX-License-Identifier: Apache-2.0
import { render, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridProps } from '@openrows/grid';
import { TableGrid } from './TableGrid';
import type { StagedEdit } from './editHistory';
import { emptyHistory } from './editHistory';
import type { RowEditing } from './useRowEditing';
import { emptyLayout } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';

vi.mock('@openrows/grid/canvas', () => ({
  mountGrid: vi.fn(() => ({ destroy: vi.fn() })),
}));

function editing(current: readonly StagedEdit[] = []): RowEditing {
  return {
    info: null,
    history: { ...emptyHistory, current },
    inline: null,
  } as unknown as RowEditing;
}

function gridProps(onRow = vi.fn()) {
  return {
    result: {
      id: 'before-commit',
      rows: 3,
      done: true,
      truncated: false,
      statement: 0,
      columns: [{ name: 'id', sourceType: 'int4' }],
    },
    sort: null,
    editing: editing(),
    offset: 0,
    rowNumbers: true,
    striped: true,
    onHeader: vi.fn(),
    onRow,
    onContextMenu: vi.fn(),
    layout: emptyLayout,
    onLayout: vi.fn(),
    copyTarget: { engine: 'postgres' as const, table: null },
  };
}

it('refreshes inspector selection when a committed page replaces the previous result', async () => {
  const onRow = vi.fn();
  const props = gridProps(onRow);
  const view = render(
    <ThemeProvider>
      <TableGrid {...props} />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(onRow).toHaveBeenCalledWith(0);
  });
  onRow.mockClear();
  view.rerender(
    <ThemeProvider>
      <TableGrid {...props} result={{ ...props.result, id: 'after-commit' }} />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(onRow).toHaveBeenCalledWith(0);
  });
  expect(mountGrid).toHaveBeenCalledTimes(2);
  view.rerender(
    <ThemeProvider>
      <TableGrid
        {...props}
        result={{ ...props.result, id: 'deleted-all', rows: 0 }}
      />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalledTimes(3);
  });
  expect(onRow).toHaveBeenCalledOnce();
});

it('keeps the selected row when edits are staged, undone and discarded', async () => {
  const onRow = vi.fn();
  const props = {
    ...gridProps(onRow),
    result: { ...gridProps().result, rows: 40 },
  };
  const edit = {
    id: '19',
    number: 19,
    kind: 'update' as const,
    original: { id: '19' },
    values: { id: { kind: 'text' as const, value: '190' } },
  };
  const view = render(
    <ThemeProvider>
      <TableGrid {...props} />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalledOnce();
  });
  const grid = vi.mocked(mountGrid).mock.calls[0]?.[1] as GridProps;
  const repaint = vi.fn();
  grid.source.subscribe(repaint);
  grid.onSelection({ row: 18, column: 1, ranges: [] });
  onRow.mockClear();
  for (const changes of [[edit], [], [edit], []])
    view.rerender(
      <ThemeProvider>
        <TableGrid {...props} editing={editing(changes)} />
      </ThemeProvider>,
    );
  expect(repaint).toHaveBeenCalledTimes(4);
  expect(mountGrid).toHaveBeenCalledOnce();
  expect(grid.selection).toMatchObject({ row: 18, column: 1 });
  expect(onRow).not.toHaveBeenCalled();
});

it('applies stored widths and pins, resizes in place and persists drops', async () => {
  vi.mocked(mountGrid).mockClear();
  const relayout = vi.fn();
  vi.mocked(mountGrid).mockImplementationOnce(() => ({
    destroy: vi.fn(),
    relayout,
    metrics: {} as never,
    viewport: { x: 0, y: 0, width: 0, height: 0 },
    scrollTo: vi.fn(),
  }));
  const props = gridProps();
  const onLayout = vi.fn();
  const layout: ColumnLayout = {
    widths: { name: 222 },
    order: [],
    pinned: ['id'],
  };
  const result = {
    ...props.result,
    columns: [
      { name: 'id', sourceType: 'int4' },
      { name: 'name', sourceType: 'text' },
    ],
  };
  const view = render(
    <ThemeProvider>
      <TableGrid
        {...props}
        result={result}
        layout={layout}
        onLayout={onLayout}
      />
    </ThemeProvider>,
  );
  await waitFor(() => {
    expect(mountGrid).toHaveBeenCalledOnce();
  });
  const grid = vi.mocked(mountGrid).mock.calls[0]?.[1] as GridProps;
  // Row numbers lead, then the pinned `id`, then `name` at its stored width.
  expect(
    grid.source.columns.map((column) => [column.width, column.pinned]),
  ).toEqual([
    [48, true],
    [expect.any(Number), true],
    [222, false],
  ]);
  grid.onColumnResize?.(2, 150, false);
  expect(grid.source.columns[2]?.width).toBe(150);
  expect(relayout).toHaveBeenCalledOnce();
  expect(onLayout).not.toHaveBeenCalled();
  grid.onColumnResize?.(2, 160, true);
  const widthUpdate = onLayout.mock.calls[0]?.[0] as (
    l: ColumnLayout,
  ) => ColumnLayout;
  expect(widthUpdate(layout).widths).toEqual({ name: 160 });
  grid.onColumnMove?.(2, 1);
  const moveUpdate = onLayout.mock.calls[1]?.[0] as (
    l: ColumnLayout,
  ) => ColumnLayout;
  // `name` cannot enter the pinned region; the order stays valid.
  expect(moveUpdate(layout).order).toEqual(['id', 'name']);
  // Unpinning the leading column needs no reload: it applies in place.
  relayout.mockClear();
  view.rerender(
    <ThemeProvider>
      <TableGrid
        {...props}
        result={result}
        layout={{ ...layout, pinned: [] }}
        onLayout={onLayout}
      />
    </ThemeProvider>,
  );
  expect(grid.source.columns[1]?.pinned).toBe(false);
  expect(relayout).toHaveBeenCalledOnce();
  expect(mountGrid).toHaveBeenCalledOnce();
});
