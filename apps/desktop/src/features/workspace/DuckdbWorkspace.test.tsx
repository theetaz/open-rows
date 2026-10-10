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
import {
  listDatabases,
  listSchemas,
  listObjects,
  openSession,
} from '@openrows/ipc';
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
it('keeps query scope while refusing hidden catalog reads and unavailable toolbar actions', async () => {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.click(
    await screen.findByRole('button', { name: 'Switch to local-duckdb' }),
  );
  await screen.findByText(
    'DuckDB catalog browsing is not available yet. Open a query tab to run SQL.',
  );
  await waitFor(() => {
    expect(openSession).toHaveBeenCalledWith('id-local-duckdb', 'main');
  });
  expect(listDatabases).not.toHaveBeenCalled();
  expect(listSchemas).not.toHaveBeenCalled();
  expect(listObjects).not.toHaveBeenCalled();
  expect(
    screen.getByRole('button', { name: 'Open database (⌘K)' }),
  ).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Refresh' })).toBeDisabled();
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
