// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  onMenuCommand,
  openConnection,
  openSession,
  syncMenu,
} from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from './Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
  vi.mocked(openConnection).mockClear();
  vi.mocked(openSession).mockClear();
});

function mount(): void {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
}

/** A table tab on the prod connection, as the last session stored it. */
const storedTabs = {
  tabs: [
    {
      id: 'table-1',
      kind: 'table',
      connectionId: 'id-analytics-prod',
      database: 'orders',
      title: 'invoices',
      sql: '',
      table: { database: 'orders', schema: 'public', name: 'invoices' },
    },
  ],
  active: { 'id-analytics-prod\norders': 'table-1' },
  next: 2,
};

it('restores the last session without auto-connecting production', async () => {
  localStorage.setItem(
    'openrows.session',
    JSON.stringify({
      active: 'id-analytics-prod',
      connections: [
        {
          id: 'id-orders-local',
          databases: ['orders', 'sales'],
          database: 'sales',
        },
        { id: 'id-analytics-prod', databases: ['orders'], database: 'orders' },
      ],
    }),
  );
  localStorage.setItem('openrows.queryTabs', JSON.stringify(storedTabs));
  mount();
  await waitFor(() => {
    expect(openSession).toHaveBeenCalledWith('id-orders-local', 'sales');
  });
  expect(openConnection).toHaveBeenCalledWith('id-orders-local');
  expect(openConnection).not.toHaveBeenCalledWith('id-analytics-prod');
  expect(await screen.findByRole('tab', { name: /invoices/ })).toBeVisible();
  const bar = screen.getByText(/analytics-prod is not connected/);
  expect(bar).toHaveTextContent('asks before writes');
  fireEvent.click(screen.getByRole('button', { name: 'Connect' }));
  await waitFor(() => {
    expect(openConnection).toHaveBeenCalledWith('id-analytics-prod');
  });
  await waitFor(() => {
    expect(
      JSON.parse(localStorage.getItem('openrows.session') ?? '{}'),
    ).toMatchObject({ active: 'id-analytics-prod' });
  });
});

it('starts empty when session restore is off', async () => {
  localStorage.setItem(
    'openrows.general',
    JSON.stringify({ restoreSession: false }),
  );
  localStorage.setItem(
    'openrows.session',
    JSON.stringify({
      active: 'id-orders-local',
      connections: [
        { id: 'id-orders-local', databases: ['orders'], database: 'orders' },
      ],
    }),
  );
  localStorage.setItem('openrows.queryTabs', JSON.stringify(storedTabs));
  mount();
  await screen.findByRole('button', { name: 'Switch to orders-local' });
  expect(openConnection).not.toHaveBeenCalled();
  expect(screen.queryByRole('tab', { name: /invoices/ })).toBeNull();
});

it('pins, closes others and to the right from the tab menu, and reorders by drag', async () => {
  mount();
  await connectTo();
  for (let count = 0; count < 4; count += 1)
    fireEvent.click(screen.getByRole('button', { name: 'New tab' }));
  const labels = (): string[] =>
    screen.getAllByRole('tab').map((tab) => tab.textContent);
  expect(labels()).toEqual([
    '›_SQL Query 1',
    '›_SQL Query 2',
    '›_SQL Query 3',
    '›_SQL Query 4',
  ]);
  const menuFor = (name: string): void => {
    fireEvent.contextMenu(screen.getByRole('tab', { name: new RegExp(name) }));
  };
  menuFor('SQL Query 3');
  fireEvent.click(screen.getByRole('menuitem', { name: 'Pin Tab' }));
  expect(labels()[0]).toBe('›_SQL Query 3');
  expect(screen.getByRole('img', { name: 'Pinned' })).toBeInTheDocument();
  menuFor('SQL Query 2');
  fireEvent.click(
    screen.getByRole('menuitem', { name: 'Close Tabs to the Right' }),
  );
  expect(labels()).toEqual(['›_SQL Query 3', '›_SQL Query 1', '›_SQL Query 2']);
  const first = screen.getByRole('tab', { name: /SQL Query 1/ });
  fireEvent.pointerDown(first.parentElement ?? first, {
    button: 0,
    clientX: 0,
  });
  fireEvent.pointerMove(window, { clientX: 40 });
  fireEvent.pointerUp(window, { clientX: 40 });
  // The click that ends a drag does not select the dragged tab.
  fireEvent.click(first);
  expect(first).toHaveAttribute('aria-selected', 'false');
  expect(labels()).toEqual(['›_SQL Query 3', '›_SQL Query 2', '›_SQL Query 1']);
  menuFor('SQL Query 1');
  fireEvent.click(screen.getByRole('menuitem', { name: 'Close Other Tabs' }));
  expect(labels()).toEqual(['›_SQL Query 3', '›_SQL Query 1']);
});

it('renames a tab from the tab menu and lists every tab when they overflow', async () => {
  mount();
  await connectTo();
  for (let count = 0; count < 3; count += 1)
    fireEvent.click(screen.getByRole('button', { name: 'New tab' }));
  fireEvent.contextMenu(screen.getByRole('tab', { name: /SQL Query 2/ }));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Rename Tab…' }));
  const field = screen.getByRole('textbox', { name: 'Tab name' });
  fireEvent.change(field, { target: { value: 'Monthly totals' } });
  fireEvent.keyDown(field, { key: 'Enter' });
  expect(screen.getByRole('tab', { name: /Monthly totals/ })).toBeVisible();
  expect(screen.queryByRole('button', { name: 'All tabs' })).toBeNull();
  vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(900);
  fireEvent.click(screen.getByRole('button', { name: 'New tab' }));
  fireEvent.click(screen.getByRole('button', { name: 'All tabs' }));
  const items = screen.getAllByRole('menuitemradio');
  expect(items.map((item) => item.textContent)).toEqual([
    'SQL Query 1',
    'Monthly totals',
    'SQL Query 3',
    '✓SQL Query 4',
  ]);
  fireEvent.click(screen.getByRole('menuitemradio', { name: /Monthly/ }));
  expect(screen.getByRole('tab', { name: /Monthly totals/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  vi.restoreAllMocks();
});

it('routes native menu commands and syncs shortcuts and enabled state', async () => {
  let menu: ((id: string) => void) | undefined;
  vi.mocked(onMenuCommand).mockImplementation((listener) => {
    menu = listener;
    return Promise.resolve(() => undefined);
  });
  mount();
  await connectTo();
  await waitFor(() => {
    expect(menu).toBeDefined();
  });
  act(() => {
    menu?.('new-tab');
  });
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await waitFor(() => {
    const states = vi.mocked(syncMenu).mock.lastCall?.[0] ?? [];
    expect(states).toContainEqual({
      id: 'run-current',
      enabled: true,
      accelerator: 'Cmd+Enter',
    });
  });
  const states = vi.mocked(syncMenu).mock.lastCall?.[0] ?? [];
  expect(states).toContainEqual({
    id: 'connect',
    enabled: false,
    accelerator: null,
  });
  act(() => {
    menu?.('close-tab');
  });
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
});
