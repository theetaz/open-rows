// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import type { BoundFunctions, queries } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { objectChange } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('@openrows/editor', async () =>
  (await import('../../test/editorStub')).editorStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
  vi.mocked(objectChange).mockClear();
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

it('drops a table from its context menu, then closes its tab', async () => {
  const user = mount();
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  await user.dblClick(orders);
  expect(await screen.findByRole('tab', { name: /orders/ })).toBeVisible();
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'orders actions' });
  expect(
    within(menu).getByRole('menuitem', { name: 'Rename Table…' }),
  ).toBeEnabled();
  await user.click(within(menu).getByRole('menuitem', { name: 'Drop Table…' }));
  const sheet = screen.getByRole('dialog', { name: 'Drop table orders' });
  const drop = within(sheet).getByRole('button', { name: 'Drop' });
  await waitFor(() => {
    expect(drop).toBeEnabled();
  });
  await user.click(drop);
  await waitFor(() => {
    expect(
      screen.queryByRole('dialog', { name: 'Drop table orders' }),
    ).toBeNull();
  });
  expect(screen.queryByRole('tab', { name: /^orders/ })).toBeNull();
});

it('edits and drops routines from their context menu', async () => {
  const user = mount();
  await connectTo();
  await user.click(sidebar().getByRole('button', { name: /^Functions/ }));
  const calc = await sidebar().findByRole('treeitem', { name: /calc_tax/ });
  fireEvent.contextMenu(calc, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'calc_tax actions' });
  expect(within(menu).queryByRole('menuitem', { name: 'Export…' })).toBeNull();
  await user.click(
    within(menu).getByRole('menuitem', { name: 'Drop Function…' }),
  );
  const sheet = screen.getByRole('dialog', { name: 'Drop function calc_tax' });
  await waitFor(() => {
    expect(vi.mocked(objectChange).mock.lastCall?.[0].change).toEqual({
      kind: 'dropRoutine',
      target: { schema: 'public', name: 'calc_tax' },
      routine: 'function',
      arguments: 'total numeric',
    });
  });
  fireEvent.keyDown(sheet, { key: 'Escape' });
  expect(screen.queryByRole('dialog', { name: /calc_tax/ })).toBeNull();
});

it('creates objects from the sidebar + menu and the command palette', async () => {
  const user = mount();
  await connectTo();
  await user.click(sidebar().getByRole('button', { name: 'Create object' }));
  const menu = await screen.findByRole('menu', { name: 'Create' });
  expect(
    within(menu).getByRole('menuitem', { name: 'New Materialized View…' }),
  ).toBeEnabled();
  expect(
    within(menu).getByRole('menuitem', { name: 'Drop Database…' }),
  ).toBeEnabled();
  await user.click(within(menu).getByRole('menuitem', { name: 'New Table…' }));
  const table = screen.getByRole('dialog', { name: 'New table in public' });
  fireEvent.keyDown(table, { key: 'Escape' });
  await act(async () => {
    fireEvent.keyDown(window, { key: 'p', metaKey: true, shiftKey: true });
    await import('./WorkspacePalettes');
  });
  const palette = within(
    screen.getByRole('dialog', { name: 'Command palette' }),
  );
  fireEvent.change(palette.getByRole('combobox'), {
    target: { value: 'New schema' },
  });
  fireEvent.keyDown(palette.getByRole('combobox'), { key: 'Enter' });
  const schema = await screen.findByRole('dialog', { name: 'New schema' });
  await user.type(
    within(schema).getByRole('textbox', { name: 'Name' }),
    'audit',
  );
  await waitFor(() => {
    expect(vi.mocked(objectChange).mock.lastCall?.[0]).toMatchObject({
      database: 'orders',
      change: { kind: 'createSchema', name: 'audit' },
    });
  });
});
