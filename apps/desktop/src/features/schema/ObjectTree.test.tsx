// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { BoundFunctions, queries } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  listObjects,
  listSchemas,
  schemaDiagram,
  startExport,
} from '@openrows/ipc';
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

it('explains the empty tree before a connection is selected', () => {
  mount();
  expect(
    sidebar().getByText('Choose a connection to browse its objects.'),
  ).toBeVisible();
  expect(listSchemas).not.toHaveBeenCalled();
});

it('loads tables at once and other sections only when expanded', async () => {
  const user = mount();
  await connectTo();
  expect(listSchemas).toHaveBeenCalledWith('id-orders-local', 'orders', false);
  expect(listObjects).toHaveBeenCalledWith(
    'id-orders-local',
    'orders',
    { schema: 'public', kinds: ['table', 'foreignTable'], limit: 5000 },
    false,
  );
  expect(
    await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' }),
  ).toBeVisible();
  expect(listObjects).toHaveBeenCalledOnce();
  const views = sidebar().getByRole('button', { name: /^Views/ });
  expect(views).not.toHaveTextContent('2');
  await user.click(views);
  expect(listObjects).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders',
    { schema: 'public', kinds: ['view'], limit: 5000 },
    false,
  );
  expect(
    await sidebar().findByRole('treeitem', { name: 'order_summary' }),
  ).toBeVisible();
  expect(views).toHaveTextContent('Views2');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Schema' }),
    'archive',
  );
  expect(
    await sidebar().findByRole('treeitem', { name: 'session_notes ~16' }),
  ).toBeVisible();
});

it('search filters loaded names, highlights matches, shows No matches and clears with Esc', async () => {
  const user = mount();
  await connectTo();
  await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' });
  const input = screen.getByRole('textbox', { name: 'Search items' });
  fireEvent.change(input, { target: { value: 'summary' } });
  const match = await sidebar().findByRole('treeitem', {
    name: 'order_summary',
  });
  expect(match.querySelectorAll('mark')).toHaveLength(7);
  expect(sidebar().queryByRole('treeitem', { name: /order_items/ })).toBeNull();
  expect(sidebar().getByRole('button', { name: /^Views/ })).toHaveAttribute(
    'aria-expanded',
    'true',
  );
  fireEvent.change(input, { target: { value: 'no_such_item' } });
  expect(await sidebar().findByText('No matches')).toBeVisible();
  fireEvent.keyDown(input, { key: 'Escape' });
  expect(input).toHaveValue('');
  expect(sidebar().getByRole('button', { name: /^Views/ })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  const tables = sidebar().getByRole('button', { name: /^Tables/ });
  tables.focus();
  await user.keyboard('{Meta>}f{/Meta}');
  expect(input).toHaveFocus();
});

it('carries ArrowDown and ArrowUp from the last or first row of one section into the next', async () => {
  const user = mount();
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  orders.focus();
  await user.keyboard('{ArrowDown}');
  const views = sidebar().getByRole('button', { name: /^Views/ });
  expect(views).toHaveFocus();
  await user.click(views);
  const first = await sidebar().findByRole('treeitem', {
    name: 'active_customers',
  });
  orders.focus();
  await user.keyboard('{ArrowDown}');
  expect(first).toHaveFocus();
  await user.keyboard('{ArrowUp}');
  expect(orders).toHaveFocus();
});

it('shows a section error with Retry, and ⌘R in the sidebar re-reads the catalog', async () => {
  vi.mocked(listObjects).mockRejectedValueOnce(
    new Error('permission denied for schema public'),
  );
  const user = mount();
  await connectTo();
  const alert = await sidebar().findByRole('alert');
  expect(alert).toHaveTextContent('permission denied for schema public');
  await user.click(within(alert).getByRole('button', { name: 'Retry' }));
  expect(
    await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' }),
  ).toBeVisible();
  const calls = vi.mocked(listSchemas).mock.calls.length;
  sidebar()
    .getByRole('button', { name: /^Tables/ })
    .focus();
  await user.keyboard('{Meta>}r{/Meta}');
  await sidebar().findByRole('treeitem', { name: 'orders ~2.4M' });
  expect(vi.mocked(listSchemas).mock.calls.length).toBe(calls + 1);
});

it('exports a table from its context menu on the database session', async () => {
  const user = mount();
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'orders actions' });
  await user.click(within(menu).getByRole('menuitem', { name: 'Export…' }));
  const sheet = await screen.findByRole('dialog', { name: 'Export orders' });
  await user.click(within(sheet).getByRole('button', { name: 'Export…' }));
  expect(startExport).toHaveBeenCalledWith(
    {
      source: {
        kind: 'table',
        sessionId: 'session-id-orders-local-orders',
        target: {
          profileId: 'id-orders-local',
          database: 'orders',
          schema: 'public',
          table: 'orders',
        },
      },
      format: {
        kind: 'csv',
        delimiter: 'comma',
        options: {
          quote: 'double',
          header: true,
          nullText: '',
          lineEnding: 'crlf',
          encoding: 'utf8',
        },
      },
      fileName: 'orders',
    },
    expect.any(Function),
  );
});

it('imports into a table from its context menu', async () => {
  const user = mount();
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'orders actions' });
  await user.click(within(menu).getByRole('menuitem', { name: 'Import…' }));
  const sheet = await screen.findByRole('dialog', {
    name: 'Import into orders.public',
  });
  await user.click(within(sheet).getByRole('button', { name: 'Choose File…' }));
  expect(await within(sheet).findByLabelText('Table in public')).toHaveValue(
    'orders',
  );
});

it('lists large sections 5,000 objects at a time', async () => {
  const table = (name: string) => ({
    schema: 'public',
    name,
    kind: 'table' as const,
    arguments: null,
    rowEstimate: null,
    comment: null,
  });
  vi.mocked(listObjects).mockImplementation((_profile, _database, section) =>
    Promise.resolve({
      data:
        section.limit === 5000
          ? [table('first')]
          : [table('first'), table('second')],
      total: 7500,
      stale: false,
    }),
  );
  const user = mount();
  await connectTo();
  const more = await sidebar().findByRole('button', {
    name: 'Show 5,000 more of 7,499',
  });
  expect(sidebar().getByRole('button', { name: /^Tables/ })).toHaveTextContent(
    '7500',
  );
  await user.click(more);
  expect(listObjects).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders',
    { schema: 'public', kinds: ['table', 'foreignTable'], limit: 10000 },
    false,
  );
  expect(await sidebar().findByText('second')).toBeVisible();
  vi.mocked(listObjects).mockReset();
});

it("shows the ER diagram of a table's schema and opens a table from it", async () => {
  const user = mount();
  await connectTo();
  const orders = await sidebar().findByRole('treeitem', {
    name: 'orders ~2.4M',
  });
  fireEvent.contextMenu(orders, { clientX: 40, clientY: 80 });
  const menu = await screen.findByRole('menu', { name: 'orders actions' });
  await user.click(
    within(menu).getByRole('menuitem', { name: 'Show ER Diagram' }),
  );
  const sheet = await screen.findByRole('dialog', {
    name: 'ER Diagram — public',
  });
  expect(schemaDiagram).toHaveBeenCalledWith({
    profileId: 'id-orders-local',
    database: 'orders',
    schema: 'public',
  });
  const customers = await within(sheet).findByRole('button', {
    name: 'Open table customers',
  });
  await user.click(customers);
  expect(sheet).not.toBeInTheDocument();
  expect(await screen.findByRole('tab', { name: /customers/ })).toBeVisible();
});
