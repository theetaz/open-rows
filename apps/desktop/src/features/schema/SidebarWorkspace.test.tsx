// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { listObjects } from '@openrows/ipc';
import { Workspace } from '../workspace/Workspace';
import { connectTo } from '../../test/connect';
import { readSidebarObjects } from './sidebarStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
const sidebar = () =>
  within(screen.getByRole('complementary', { name: 'Sidebar' }));

it('stars from the context menu and table header, records opens, and routes the favourite shortcut', async () => {
  vi.stubGlobal('innerWidth', 1280);
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  const reads = vi.mocked(listObjects).mock.calls.length;
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'orders actions' });
  await user.click(
    within(menu).getByRole('menuitem', { name: 'Add favourite' }),
  );
  await user.keyboard('{Escape}');
  expect(
    sidebar().getByRole('treeitem', { name: 'public.orders' }),
  ).toBeVisible();
  expect(vi.mocked(listObjects).mock.calls).toHaveLength(reads);
  await user.dblClick(orders);
  // The header follows the lazy table import, not just the tab descriptor.
  await act(async () => {
    await vi.dynamicImportSettled();
  });
  const header = screen.getByRole('button', { name: 'Remove favourite' });
  expect(readSidebarObjects('id-orders-local', 'orders').recent[0]?.name).toBe(
    'orders',
  );
  await user.click(header);
  expect(
    readSidebarObjects('id-orders-local', 'orders').favourites,
  ).toHaveLength(0);
  await user.keyboard('{Alt>}{Meta>}d{/Meta}{/Alt}');
  expect(
    readSidebarObjects('id-orders-local', 'orders').favourites[0]?.name,
  ).toBe('orders');
  vi.mocked(listObjects).mockResolvedValueOnce({
    data: [],
    total: 0,
    stale: false,
  });
  sidebar().getByRole('textbox', { name: 'Search items' }).focus();
  await user.keyboard('{Meta>}r{/Meta}');
  const favourites = sidebar().getByRole('tree', { name: 'Favourites' });
  expect(
    await within(favourites).findByRole('treeitem', {
      name: 'public.orders (missing)',
    }),
  ).toBeVisible();
  await user.click(
    within(favourites).getByRole('button', { name: 'Remove public.orders' }),
  );
  expect(readSidebarObjects('id-orders-local', 'orders').favourites).toEqual(
    [],
  );
});
