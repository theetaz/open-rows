// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import type { BoundFunctions, queries } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { tableStructure } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from '../workspace/Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});

function mount(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}

const sidebar = (): BoundFunctions<typeof queries> =>
  within(screen.getByRole('complementary', { name: 'Sidebar' }));

const chevron = (row: HTMLElement): HTMLElement => {
  const element = row.querySelector<HTMLElement>('span[aria-hidden="true"]');
  if (!element) throw new Error('row has no disclosure');
  return element;
};

async function ordersRow(): Promise<HTMLElement> {
  await connectTo();
  return sidebar().findByRole('treeitem', { name: 'orders ~2.4M' });
}

it('expands a table to its columns once, marks the primary key and collapses', async () => {
  const user = mount();
  const orders = await ordersRow();
  expect(orders).toHaveAttribute('aria-expanded', 'false');
  expect(tableStructure).not.toHaveBeenCalled();
  await user.click(chevron(orders));
  expect(orders).toHaveAttribute('aria-expanded', 'true');
  expect(orders).toHaveAttribute('aria-selected', 'false');
  expect(tableStructure).toHaveBeenCalledWith({
    profileId: 'id-orders-local',
    database: 'orders',
    schema: 'public',
    table: 'orders',
  });
  const group = within(orders).getByRole('group');
  const id = await within(group).findByRole('treeitem', {
    name: 'id integer, primary key',
  });
  expect(id).toHaveAttribute('aria-level', '2');
  expect(id.querySelector('svg')).not.toBeNull();
  const name = within(group).getByRole('treeitem', { name: 'name text' });
  expect(name.querySelector('svg')).toBeNull();
  expect(screen.queryByRole('tab', { name: /^orders/ })).toBeNull();
  await user.click(chevron(orders));
  expect(orders).toHaveAttribute('aria-expanded', 'false');
  expect(within(orders).queryByRole('group')).toBeNull();
  await user.click(chevron(orders));
  await within(orders).findByRole('treeitem', { name: 'name text' });
  expect(tableStructure).toHaveBeenCalledOnce();
  sidebar()
    .getByRole('button', { name: /^Tables/ })
    .focus();
  await user.keyboard('{Meta>}r{/Meta}');
  await sidebar().findByRole('treeitem', { name: 'name text' });
  expect(tableStructure).toHaveBeenCalledTimes(2);
});

it('shows an inline error row when columns fail and retries on re-expand', async () => {
  vi.mocked(tableStructure).mockRejectedValueOnce(
    new Error('permission denied for table orders'),
  );
  const user = mount();
  const orders = await ordersRow();
  await user.click(chevron(orders));
  expect(
    await within(orders).findByText('permission denied for table orders'),
  ).toBeVisible();
  await user.click(chevron(orders));
  await user.click(chevron(orders));
  expect(
    await within(orders).findByRole('treeitem', { name: 'name text' }),
  ).toBeVisible();
  expect(tableStructure).toHaveBeenCalledTimes(2);
});

it('moves focus to the table row when its chevron collapses a focused column', async () => {
  const user = mount();
  const orders = await ordersRow();
  orders.focus();
  await user.keyboard('{ArrowRight}');
  const id = await within(orders).findByRole('treeitem', {
    name: 'id integer, primary key',
  });
  id.focus();
  await user.click(chevron(orders));
  expect(orders).toHaveAttribute('aria-expanded', 'false');
  expect(orders).toHaveFocus();
});

it('moves through tables and columns and expands or collapses with arrow keys', async () => {
  const user = mount();
  const orders = await ordersRow();
  const customers = sidebar().getByRole('treeitem', {
    name: 'customers ~412K',
  });
  expect(customers).toHaveAttribute('tabindex', '0');
  expect(orders).toHaveAttribute('tabindex', '-1');
  customers.focus();
  await user.keyboard('{ArrowDown}{ArrowDown}');
  expect(orders).toHaveFocus();
  expect(orders).toHaveAttribute('aria-selected', 'true');
  await user.keyboard('{ArrowRight}');
  expect(orders).toHaveAttribute('aria-expanded', 'true');
  const id = await within(orders).findByRole('treeitem', {
    name: 'id integer, primary key',
  });
  await user.keyboard('{ArrowRight}');
  expect(id).toHaveFocus();
  await user.keyboard('{ArrowDown}');
  const name = within(orders).getByRole('treeitem', { name: 'name text' });
  expect(name).toHaveFocus();
  expect(name).toHaveAttribute('aria-selected', 'true');
  expect(name).toHaveAttribute('tabindex', '0');
  await user.keyboard('{ArrowLeft}');
  expect(orders).toHaveFocus();
  await user.keyboard('{ArrowLeft}');
  expect(orders).toHaveAttribute('aria-expanded', 'false');
  await user.keyboard('{Home}');
  expect(customers).toHaveFocus();
  await user.keyboard('{End}{Enter}');
  expect(orders).toHaveFocus();
  expect(
    await screen.findByRole('region', { name: 'orders table' }),
  ).toBeVisible();
});
