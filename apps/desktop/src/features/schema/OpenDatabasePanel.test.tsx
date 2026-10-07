// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { closeSession, listSchemas, openSession } from '@openrows/ipc';
import { connectTo } from '../../test/connect';
import { Workspace } from '../../mock/Workspace';

vi.mock('@openrows/ipc', async () => {
  const stub = await import('../../test/ipcStub');
  return stub.ipcStub([
    stub.profile('orders-local'),
    stub.profile('server-only', { database: null }),
  ]);
});

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});

async function mount(): Promise<void> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await connectTo();
}

async function openPanel(): Promise<HTMLElement> {
  await act(async () => {
    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await import('../../mock/WorkspacePalettes');
  });
  return screen.findByRole('dialog', { name: 'Open database' });
}

const search = (): HTMLElement =>
  screen.getByRole('combobox', { name: 'Search for database' });

it('⌘K lists databases with search focus, hides system ones until asked and filters', async () => {
  await mount();
  const panel = within(await openPanel());
  await waitFor(() => {
    expect(search()).toHaveFocus();
  });
  const names = (): string[] =>
    panel.getAllByRole('option').map((option) => option.textContent);
  expect(names()).toEqual([
    'analytics_scratch',
    'ordersOpen',
    'orders_archive',
  ]);
  expect(panel.getAllByRole('option')[0]).toHaveAttribute(
    'aria-selected',
    'true',
  );
  fireEvent.click(
    panel.getByRole('checkbox', { name: 'Show system databases' }),
  );
  expect(names()).toContain('postgres');
  fireEvent.change(search(), { target: { value: 'ARCH' } });
  expect(names()).toEqual(['orders_archive']);
  fireEvent.change(search(), { target: { value: 'nothing' } });
  expect(panel.getByRole('status')).toHaveTextContent('No matching databases');
  expect(panel.getByRole('button', { name: 'Open' })).toBeDisabled();
  const create = panel.getByRole('button', { name: 'New…' });
  expect(create).toHaveAttribute('aria-disabled', 'true');
  expect(create).toHaveAttribute('title', 'Creating databases is coming soon');
});

it('arrows and Return open a database on its own session; Esc cancels', async () => {
  await mount();
  await openPanel();
  fireEvent.keyDown(search(), { key: 'ArrowDown' });
  fireEvent.keyDown(search(), { key: 'ArrowDown' });
  fireEvent.keyDown(search(), { key: 'ArrowUp' });
  fireEvent.keyDown(search(), { key: 'ArrowUp' });
  fireEvent.keyDown(search(), { key: 'ArrowUp' });
  fireEvent.keyDown(search(), { key: 'Enter' });
  expect(screen.queryByRole('dialog', { name: 'Open database' })).toBeNull();
  expect(openSession).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders_archive',
  );
  expect(listSchemas).toHaveBeenLastCalledWith(
    'id-orders-local',
    'orders_archive',
  );
  await openPanel();
  fireEvent.keyDown(search(), { key: 'Escape' });
  expect(screen.queryByRole('dialog', { name: 'Open database' })).toBeNull();
});

it('double-click opens; rail tiles switch databases and keep per-database tab sets', async () => {
  await mount();
  fireEvent.keyDown(window, { key: 't', metaKey: true });
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toBeVisible();
  const panel = within(await openPanel());
  fireEvent.doubleClick(
    panel.getByRole('option', { name: 'analytics_scratch' }),
  );
  const rail = within(screen.getByRole('group', { name: 'Opened databases' }));
  await waitFor(() => {
    expect(
      rail.getByRole('button', { name: 'Database analytics_scratch' }),
    ).toHaveAttribute('aria-pressed', 'true');
  });
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'LOCAL | PostgreSQL 17.2 : orders-local : analytics_scratch',
  );
  fireEvent.click(rail.getByRole('button', { name: 'Database orders' }));
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toBeVisible();
  fireEvent.contextMenu(rail.getByRole('button', { name: 'Database orders' }));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Close database' }));
  await waitFor(() => {
    expect(closeSession).toHaveBeenCalledWith('session-id-orders-local-orders');
  });
  await waitFor(() => {
    expect(rail.queryByRole('button', { name: 'Database orders' })).toBeNull();
  });
  expect(
    rail.getByRole('button', { name: 'Database analytics_scratch' }),
  ).toHaveAttribute('aria-pressed', 'true');
});

it('a connection without a database asks for one and the sidebar explains ⌘K', async () => {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.click(
    await screen.findByRole('button', { name: 'Switch to server-only' }),
  );
  expect(
    await screen.findByRole('dialog', { name: 'Open database' }),
  ).toBeVisible();
  fireEvent.keyDown(search(), { key: 'Escape' });
  expect(
    screen.getByText('No database selected, press ⌘K to select a database'),
  ).toBeVisible();
  expect(openSession).not.toHaveBeenCalledWith('id-server-only', null);
});
