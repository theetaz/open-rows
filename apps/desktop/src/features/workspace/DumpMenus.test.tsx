// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { checkRestore, pickRestoreFile, previewBackup } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { showDump } from '../dump/dumpStore';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('@openrows/editor', async () =>
  (await import('../../test/editorStub')).editorStub(),
);

async function mount(): Promise<ReturnType<typeof userEvent.setup>> {
  await import('../dump/BackupSheet');
  await import('../dump/RestoreSheet');
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await connectTo();
  return userEvent.setup();
}

it('table and schema context menus open backups with the selected scope', async () => {
  const user = await mount();
  const tree = within(screen.getByRole('complementary', { name: 'Sidebar' }));
  const orders = await tree.findByRole('treeitem', { name: 'orders ~2.4M' });
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  await user.click(screen.getByRole('menuitem', { name: 'Back Up Table…' }));
  const sheet = await screen.findByRole('dialog', { name: 'Back Up orders' });
  expect(await within(sheet).findByText('pg_dump shop')).toBeVisible();
  expect(previewBackup).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({
        schemas: [],
        tables: [{ schema: 'public', name: 'orders' }],
      }) as unknown,
    }),
  );
  act(() => {
    showDump(null);
  });
  fireEvent.contextMenu(tree.getByRole('combobox', { name: 'Schema' }));
  await user.click(screen.getByRole('menuitem', { name: 'Back Up…' }));
  const schema = await screen.findByRole('dialog', { name: 'Back Up orders' });
  await within(schema).findByText('pg_dump shop');
  expect(previewBackup).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({
        schemas: ['public'],
        tables: [],
      }) as unknown,
    }),
  );
  act(() => {
    showDump(null);
  });
});

it('database and schema restore menus retain production confirmation and schema scope', async () => {
  const user = await mount();
  vi.mocked(pickRestoreFile).mockResolvedValue({
    path: '/tmp/orders.dump',
    name: 'orders.dump',
    kind: 'pgCustom',
    bytes: 4096,
  });
  vi.mocked(checkRestore).mockResolvedValue({
    commands: ['pg_restore orders'],
    warnings: [],
    blocked: null,
    database: 'orders',
    typed: true,
    confirmToken: 'test-only',
  });
  fireEvent.contextMenu(
    screen.getByRole('button', { name: 'Database orders' }),
  );
  await user.click(screen.getByRole('menuitem', { name: 'Back Up…' }));
  const backup = await screen.findByRole('dialog', { name: 'Back Up orders' });
  await within(backup).findByText('pg_dump shop');
  expect(previewBackup).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({ schemas: [], tables: [] }) as unknown,
    }),
  );
  act(() => {
    showDump(null);
  });
  fireEvent.contextMenu(
    screen.getByRole('button', { name: 'Database orders' }),
  );
  await user.click(screen.getByRole('menuitem', { name: 'Restore…' }));
  const restore = await screen.findByRole('dialog', {
    name: 'Restore orders.dump',
  });
  await within(restore).findByLabelText('Type orders to confirm the restore');
  expect(
    within(restore).getByRole('button', { name: 'Restore' }),
  ).toBeDisabled();
  expect(checkRestore).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({ schemas: [] }) as unknown,
    }),
  );
  act(() => {
    showDump(null);
  });
  fireEvent.contextMenu(screen.getByRole('combobox', { name: 'Schema' }));
  await user.click(screen.getByRole('menuitem', { name: 'Restore…' }));
  const scoped = await screen.findByRole('dialog', {
    name: 'Restore orders.dump',
  });
  await within(scoped).findByLabelText('Type orders to confirm the restore');
  expect(checkRestore).toHaveBeenLastCalledWith(
    expect.objectContaining({
      options: expect.objectContaining({ schemas: ['public'] }) as unknown,
    }),
  );
  expect(
    within(scoped).getByRole('button', { name: 'Restore' }),
  ).toBeDisabled();
  act(() => {
    showDump(null);
  });
});
