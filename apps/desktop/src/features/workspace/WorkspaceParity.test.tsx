// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  executeQuery,
  listSavedQueries,
  openConnection,
  readSqlFile,
} from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { showProject, closeProject } from '../projects/projectStore';
import { Workspace } from './Workspace';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../query/QueryEditor', async () => ({
  QueryEditor: (await import('../query/testEditor')).TextareaEditor,
}));
afterEach(() => {
  closeProject();
});
function mount(): void {
  vi.stubGlobal('innerWidth', 1280);
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
}
async function paletteCommand(name: string): Promise<void> {
  fireEvent.keyDown(window, { key: 'p', metaKey: true, shiftKey: true });
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  const dialog = await screen.findByRole('dialog', { name: 'Command palette' });
  const input = within(dialog).getByRole('combobox');
  fireEvent.change(input, { target: { value: name } });
  fireEvent.keyDown(input, { key: 'Enter' });
  await act(async () => {
    await vi.dynamicImportSettled();
  });
}
it('previews environment and safety before switching and keeps each context’s tabs', async () => {
  const user = userEvent.setup();
  mount();
  await connectTo();
  await user.click(screen.getByRole('button', { name: 'New query tab (⌘T)' }));
  await user.click(await screen.findByRole('textbox', { name: 'SQL editor' }));
  await user.paste('SELECT local_text');
  await paletteCommand('Switch Connection');
  const dialog = await screen.findByRole('dialog', {
    name: 'Switch connection',
  });
  await user.selectOptions(
    within(dialog).getByRole('combobox'),
    'id-analytics-prod',
  );
  expect(
    within(dialog).getByRole('group', { name: 'Connection preview' }),
  ).toHaveTextContent('Production connection');
  expect(dialog).toHaveTextContent('Confirm writes');
  expect(dialog).toHaveTextContent('Route: Direct → TLS → PostgreSQL');
  expect(openConnection).not.toHaveBeenCalledWith('id-analytics-prod');
  await user.click(
    within(dialog).getByRole('button', { name: 'Switch connection' }),
  );
  await waitFor(() => {
    expect(openConnection).toHaveBeenCalledWith('id-analytics-prod');
  });
  await user.click(screen.getByRole('button', { name: 'New query tab (⌘T)' }));
  await user.click(await screen.findByRole('textbox', { name: 'SQL editor' }));
  await user.paste('SELECT prod_text');
  await user.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('SELECT local_text');
  await user.click(
    screen.getByRole('button', { name: 'Switch to analytics-prod' }),
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toHaveValue('SELECT prod_text');
});
it('quick-open combines objects, local saved queries and cached project query paths', async () => {
  const user = userEvent.setup();
  vi.mocked(readSqlFile).mockClear();
  vi.mocked(listSavedQueries).mockClear();
  mount();
  await connectTo();
  act(() => {
    showProject({
      root: '/project',
      name: 'Project',
      defaultConnection: null,
      queryFolders: ['queries'],
      entries: [
        { kind: 'folder', path: 'queries' },
        { kind: 'query', path: 'queries/recent_orders.sql' },
      ],
      truncated: false,
    });
  });
  fireEvent.keyDown(window, { key: 'p', metaKey: true });
  const dialog = await screen.findByRole('dialog', { name: 'Quick open' });
  await within(dialog).findByRole('option', {
    name: /Top customers.*Local query/,
  });
  expect(
    within(dialog).getByRole('option', {
      name: /recent_orders.*Project query/,
    }),
  ).toBeVisible();
  expect(
    await within(dialog).findByRole('option', {
      name: /customers.*Table · public/,
    }),
  ).toBeVisible();
  const reads = vi.mocked(listSavedQueries).mock.calls.length;
  const input = within(dialog).getByRole('combobox');
  await user.type(input, 'Top customers');
  expect(listSavedQueries).toHaveBeenCalledTimes(reads);
  expect(readSqlFile).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: 'Enter' });
  expect(
    await screen.findByRole('tab', { name: /Top customers/ }),
  ).toBeVisible();
  expect(executeQuery).not.toHaveBeenCalled();
});
