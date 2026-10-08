// SPDX-License-Identifier: Apache-2.0
import { render, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import type { GridProps } from '@openrows/grid';
import { TableGrid } from './TableGrid';
import type { StagedEdit } from './editHistory';
import { emptyEdits } from './editHistory';
import type { RowEditing } from './useRowEditing';

vi.mock('@openrows/grid/canvas', () => ({
  mountGrid: vi.fn(() => ({ destroy: vi.fn() })),
}));

function editing(current: readonly StagedEdit[] = []): RowEditing {
  return {
    info: null,
    history: { ...emptyEdits, current },
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
