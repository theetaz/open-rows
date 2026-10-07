// SPDX-License-Identifier: Apache-2.0
import { render, waitFor } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { mountGrid } from '@openrows/grid/canvas';
import { TableGrid } from './TableGrid';

vi.mock('@openrows/grid/canvas', () => ({
  mountGrid: vi.fn(() => ({ destroy: vi.fn() })),
}));

it('refreshes inspector selection when a committed page replaces the previous result', async () => {
  const onRow = vi.fn();
  const props = {
    result: {
      id: 'before-commit',
      rows: 3,
      done: true,
      truncated: false,
      statement: 0,
      columns: [{ name: 'id', sourceType: 'int4' }],
    },
    sort: null,
    changes: [],
    editKeys: ['id'],
    offset: 0,
    rowNumbers: true,
    striped: true,
    onHeader: vi.fn(),
    onRow,
    onContextMenu: vi.fn(),
  };
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
