// SPDX-License-Identifier: Apache-2.0
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
} from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { listSchemas, openSession } from '@openrows/ipc';
import { Workspace } from './Workspace';
vi.mock('@openrows/ipc', async () => {
  const { ipcStub, profile } = await import('../../test/ipcStub');
  return ipcStub([
    profile('local-duckdb', {
      engine: 'duckdb',
      database: 'main',
      processList: false,
      objects: {
        tables: false,
        views: false,
        materializedViews: false,
        triggers: false,
        routines: false,
        databases: false,
        renameDatabases: false,
        schemas: false,
        foreignKeys: false,
        checks: false,
        comments: false,
        collations: false,
        charsets: false,
        identity: false,
        partialIndexes: false,
        keys: false,
        typeUsing: false,
        concurrentIndexes: false,
        columnReordering: false,
      },
      file: {
        path: '/test-only.duckdb',
        memory: false,
        readOnly: false,
        create: false,
        attached: [],
      },
    }),
  ]);
});
it('browses the catalog and shares one session across query tabs', async () => {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.click(
    await screen.findByRole('button', { name: 'Switch to local-duckdb' }),
  );
  await waitFor(() => {
    expect(openSession).toHaveBeenCalledWith('id-local-duckdb', 'main');
  });
  await waitFor(() => {
    expect(listSchemas).toHaveBeenCalled();
  });
  expect(
    screen.queryByText(/DuckDB catalog browsing is not available/),
  ).toBeNull();
  expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled();
  expect(screen.getByRole('button', { name: 'New SQL query' })).toBeEnabled();
  expect(screen.queryByRole('button', { name: 'Explain' })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: 'New SQL query' }));
  await act(async () => vi.dynamicImportSettled());
  await screen.findByRole('tab', { name: /SQL Query 1/ });
  fireEvent.click(screen.getByRole('button', { name: 'New SQL query' }));
  await act(async () => vi.dynamicImportSettled());
  await screen.findByRole('tab', { name: /SQL Query 2/ });
  expect(openSession).toHaveBeenCalledTimes(1);
});
