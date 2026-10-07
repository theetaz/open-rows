// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { listSchemas } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from '../../mock/Workspace';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});
afterEach(() => {
  vi.useRealTimers();
});

async function mount(): Promise<void> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await connectTo();
  vi.useFakeTimers();
}

async function key(key: string): Promise<void> {
  await act(async () => {
    fireEvent.keyDown(window, { key, metaKey: true });
    await import('../../mock/WorkspacePalettes');
  });
}

async function pick(name: string): Promise<void> {
  const input = screen.getByRole('combobox', { name: 'Switch database' });
  fireEvent.change(input, { target: { value: name } });
  fireEvent.keyDown(input, { key: 'Enter' });
  await act(async () => {
    vi.advanceTimersByTime(160);
    await Promise.resolve();
  });
}

it('⌘K lists server databases, marks open ones and hides system databases until requested', async () => {
  await mount();
  await key('k');
  const dialog = within(
    screen.getByRole('dialog', { name: 'Switch database' }),
  );
  expect(
    dialog.getByRole('option', { name: /orders.*Already open/ }),
  ).toBeVisible();
  expect(
    dialog.getByRole('option', { name: /orders_archive.*Available/ }),
  ).toBeVisible();
  expect(dialog.queryByRole('option', { name: /postgres/ })).toBeNull();
  fireEvent.click(
    dialog.getByRole('checkbox', { name: 'Show system databases' }),
  );
  expect(dialog.getByRole('option', { name: /postgres/ })).toBeVisible();
});

it('switching database opens its own catalog and keeps per-database tab sets', async () => {
  await mount();
  await key('t');
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toBeVisible();
  await key('k');
  await pick('orders_archive');
  expect(listSchemas).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders_archive',
  );
  expect(
    screen.getByRole('button', { name: 'Opened databases (orders_archive)' }),
  ).toBeVisible();
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'orders-local › orders_archive › public',
  );
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  expect(
    sidebar.getByRole('button', { name: 'orders_2024 ~820K' }),
  ).toBeVisible();
  fireEvent.click(
    screen.getByRole('button', { name: 'Opened databases (orders_archive)' }),
  );
  fireEvent.click(
    screen.getByRole('menuitemradio', { name: 'Activate database orders' }),
  );
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toBeVisible();
  fireEvent.click(
    screen.getByRole('button', { name: 'Opened databases (orders)' }),
  );
  fireEvent.click(
    screen.getByRole('menuitem', { name: 'Close database orders_archive' }),
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Opened databases (orders)' }),
  );
  expect(screen.getAllByRole('menuitemradio')).toHaveLength(1);
});
