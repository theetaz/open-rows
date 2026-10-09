// SPDX-License-Identifier: Apache-2.0
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  executeQuery,
  openConnection,
  openSession,
  searchHistory,
  tablePage,
} from '@openrows/ipc';
import { connectTo } from '../test/connect';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
  vi.mocked(tablePage).mockClear();
});
function mount(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}
it('connects saved profiles from the rail and forces safe mode for prod', async () => {
  const user = mount();
  const safe = screen.getByRole('button', { name: 'Safe mode' });
  expect(safe).toHaveAttribute('aria-pressed', 'false');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'false');
  const context = screen.getByLabelText('Connection context');
  expect(context).toHaveTextContent('Not connected : No connection');
  await user.click(
    await screen.findByRole('button', { name: 'Switch to analytics-prod' }),
  );
  expect(openConnection).toHaveBeenCalledWith('id-analytics-prod');
  await waitFor(() => {
    expect(context).toHaveTextContent(
      'PROD | PostgreSQL 17.2 : analytics-prod : orders : public',
    );
  });
  expect(openSession).toHaveBeenCalledWith('id-analytics-prod', 'orders');
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  await user.click(
    screen.getByRole('button', { name: 'Switch to events-dev' }),
  );
  await waitFor(() => {
    expect(context).toHaveTextContent(
      'DEV | PostgreSQL 17.2 : events-dev : orders',
    );
  });
  expect(safe).toHaveAttribute('aria-pressed', 'false');
  await screen.findByRole('option', { name: 'archive' });
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Schema' }),
    'archive',
  );
  expect(context).toHaveTextContent('archive');
});
/** A single click opens a preview tab, which the next single click replaces. */
async function expectPreview(
  user: ReturnType<typeof userEvent.setup>,
  orders: HTMLElement,
): Promise<void> {
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  await user.click(orders);
  expect(orders).toHaveAttribute('aria-selected', 'true');
  const preview = await screen.findByRole('tab', { name: /^orders/ });
  expect(preview.parentElement).toHaveAttribute('data-preview', 'true');
  await user.click(sidebar.getByRole('treeitem', { name: /^customers/ }));
  expect(screen.queryByRole('tab', { name: /^orders/ })).toBeNull();
  expect(
    await screen.findByRole('tab', { name: /^customers/ }),
  ).toBeInTheDocument();
}
it('switches sidebar segments, lazily expands real sections and opens objects on double-click', async () => {
  const user = mount();
  await connectTo();
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  for (const name of [
    'Views',
    'Materialized views',
    'Functions',
    'Sequences',
  ]) {
    const section = sidebar.getByRole('button', {
      name: new RegExp(`^${name}`),
    });
    expect(section).toHaveAttribute('aria-expanded', 'false');
    await user.click(section);
    expect(section).toHaveAttribute('aria-expanded', 'true');
  }
  expect(
    await sidebar.findByRole('treeitem', { name: 'calc_tax(total numeric)' }),
  ).toBeVisible();
  await user.type(
    screen.getByRole('textbox', { name: 'Search items' }),
    'customers',
  );
  expect(sidebar.queryByRole('treeitem', { name: /order_items/ })).toBeNull();
  const customers = sidebar.getByRole('treeitem', { name: 'customers ~412K' });
  expect(
    within(customers).getByTitle('orders.public.customers · Table'),
  ).toBeVisible();
  await user.clear(screen.getByRole('textbox', { name: 'Search items' }));
  await user.click(sidebar.getByRole('radio', { name: 'Queries' }));
  expect(
    await sidebar.findByRole('button', { name: /Top customers/ }),
  ).toBeVisible();
  await user.click(sidebar.getByRole('radio', { name: 'History' }));
  expect(
    await sidebar.findByText(
      /No statements run yet|Select a connection to see its history\./,
    ),
  ).toBeVisible();
  await user.click(sidebar.getByRole('radio', { name: 'Items' }));
  const orders = sidebar.getByRole('treeitem', { name: 'orders ~2.4M' });
  await expectPreview(user, orders);
  await user.dblClick(orders);
  expect(
    await screen.findByRole('region', { name: 'orders table' }),
  ).toBeVisible();
  expect(tablePage).toHaveBeenCalledWith(
    {
      sessionId: 'session-id-orders-local-orders',
      offset: 0,
      limit: 300,
      query: {
        target: {
          profileId: 'id-orders-local',
          database: 'orders',
          schema: 'public',
          table: 'orders',
        },
        columns: [],
        filters: [],
        combinator: 'and',
        sort: null,
      },
    },
    expect.any(Function),
  );
  expect(screen.getByRole('tab', { name: /^orders/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await user.dblClick(orders);
  expect(screen.getAllByRole('tab', { name: /^orders/ })).toHaveLength(1);
});
it('opens objects of another database in its own tab set on its own session', async () => {
  const user = mount();
  await connectTo();
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  fireEvent.keyDown(window, { key: 'k', metaKey: true });
  const input = await screen.findByRole('combobox', {
    name: 'Search for database',
  });
  fireEvent.change(input, { target: { value: 'orders_archive' } });
  fireEvent.keyDown(input, { key: 'Enter' });
  expect(openSession).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders_archive',
  );
  await user.dblClick(
    await screen.findByRole('treeitem', { name: 'orders_2024 ~820K' }),
  );
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  expect(screen.getByRole('tab', { name: /orders_2024/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(
    await screen.findByRole('region', { name: 'orders_2024 table' }),
  ).toBeVisible();
  await waitFor(() => {
    expect(tablePage).toHaveBeenCalled();
  });
  const [request] = vi.mocked(tablePage).mock.lastCall ?? [];
  expect(request?.sessionId).toBe('session-id-orders-local-orders_archive');
  expect(request?.query.target).toEqual({
    profileId: 'id-orders-local',
    database: 'orders_archive',
    schema: 'public',
    table: 'orders_2024',
  });
  expect(screen.queryByText(/browse-only/)).toBeNull();
  await user.click(screen.getByRole('button', { name: /^Sequences/ }));
  await user.dblClick(
    await screen.findByRole('treeitem', { name: 'orders_2024_id_seq' }),
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toHaveTextContent(
    'SELECT * FROM "public"."orders_2024_id_seq" LIMIT 1000;',
  );
  vi.mocked(executeQuery).mockResolvedValueOnce({
    kind: 'blocked',
    message: 'stop here',
    statements: [],
  });
  await user.click(
    await screen.findByRole('button', { name: 'Run current statement' }),
  );
  expect(executeQuery).toHaveBeenLastCalledWith(
    expect.objectContaining({
      sessionId: 'session-id-orders-local-orders_archive',
    }),
    expect.any(Function),
  );
});
it('opens, switches and closes query tabs per connection', async () => {
  const user = mount();
  expect(
    screen.getByText('Choose a connection in the sidebar to open a query tab.'),
  ).toBeVisible();
  await user.click(
    await screen.findByRole('button', { name: 'Switch to orders-local' }),
  );
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  await user.click(screen.getByRole('button', { name: 'New SQL query' }));
  expect(screen.getByRole('tab', { name: /SQL Query 2/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toBeVisible();
  await user.click(screen.getByRole('tab', { name: /SQL Query 1/ }));
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await user.click(
    screen.getByRole('button', { name: 'Switch to events-dev' }),
  );
  expect(screen.queryByRole('tab', { name: /SQL Query/ })).toBeNull();
  await user.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
  const tab = screen.getByRole('tab', { name: /SQL Query 1/ });
  fireEvent(tab, new MouseEvent('auxclick', { button: 1, bubbles: true }));
  await user.click(screen.getByRole('button', { name: 'Close SQL Query 2' }));
  expect(screen.queryByRole('tab', { name: /SQL Query/ })).toBeNull();
  expect(
    screen.getByRole('button', { name: 'New query tab (⌘T)' }),
  ).toBeVisible();
});
it('lists the connection history in the sidebar and opens a statement in a query tab', async () => {
  vi.mocked(searchHistory).mockResolvedValue([
    {
      id: 1,
      profileId: 'id-orders-local',
      database: 'orders',
      statement: 'SELECT 42',
      startedAtMs: 0,
      durationMs: 3,
      status: 'ok',
      rows: 1,
      errorMessage: null,
    },
  ]);
  const user = mount();
  await user.click(
    await screen.findByRole('button', { name: 'Switch to orders-local' }),
  );
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  await user.click(sidebar.getByRole('radio', { name: 'History' }));
  const history = await screen.findByRole('list', { name: 'Query history' });
  expect(searchHistory).toHaveBeenCalledWith({
    profileId: 'id-orders-local',
    text: '',
    limit: 300,
  });
  await user.click(within(history).getByRole('button', { name: /^SELECT 42/ }));
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});
