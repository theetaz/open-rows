// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';
function mount(): void {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
}
function key(key: string, shiftKey = false): void {
  fireEvent.keyDown(window, { key, metaKey: true, shiftKey });
}
function pickDatabase(name: string): void {
  key('k');
  fireEvent.change(screen.getByRole('combobox', { name: 'Switch database' }), {
    target: { value: name },
  });
  fireEvent.keyDown(screen.getByRole('combobox', { name: 'Switch database' }), {
    key: 'Enter',
  });
  act(() => {
    vi.advanceTimersByTime(160);
  });
}
afterEach(() => {
  vi.useRealTimers();
});
it('opens databases by keyboard, updates rail/context/sidebar and preserves independent tab sets', () => {
  mount();
  key('t');
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'title',
    'orders.public.SQL Query 1',
  );
  pickDatabase('orders_archive');
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'orders-staging › orders_archive › public › orders_2024',
  );
  expect(
    screen.getByRole('button', { name: 'Activate database orders_archive' }),
  ).toHaveAttribute('aria-pressed', 'true');
  expect(screen.getByRole('tab', { name: /orders_2024/ })).toHaveAttribute(
    'title',
    'orders_archive.public.orders_2024',
  );
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  key('t');
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  expect(
    sidebar.getByRole('button', { name: /archived_customers/ }),
  ).toBeVisible();
  expect(sidebar.queryByRole('button', { name: /order_items/ })).toBeNull();
  fireEvent.click(
    screen.getByRole('button', { name: 'Activate database orders' }),
  );
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Activate database orders_archive' }),
  );
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'title',
    'orders_archive.public.SQL Query 1',
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Switch to analytics-prod' }),
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Switch to orders-staging' }),
  );
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'title',
    'orders_archive.public.SQL Query 1',
  );
  fireEvent.click(
    screen.getByRole('button', { name: 'Close database orders_archive' }),
  );
  expect(
    screen.getByRole('button', { name: 'Activate database orders' }),
  ).toHaveAttribute('aria-pressed', 'true');
  expect(
    screen.queryByRole('button', { name: 'Activate database orders_archive' }),
  ).toBeNull();
});
it('context opens the database switcher, marks opened entries and hides system databases until requested', () => {
  mount();
  const opener = screen.getByRole('button', {
    name: 'Switch database (orders)',
  });
  opener.focus();
  fireEvent.click(opener);
  expect(
    screen.getByRole('option', { name: /orders.*Already open/ }),
  ).toBeVisible();
  expect(screen.queryByRole('option', { name: /postgres/ })).toBeNull();
  fireEvent.click(
    screen.getByRole('checkbox', { name: 'Show system databases' }),
  );
  const input = screen.getByRole('combobox', { name: 'Switch database' });
  fireEvent.change(input, { target: { value: 'template1' } });
  expect(
    within(screen.getByRole('dialog')).getAllByRole('option'),
  ).toHaveLength(1);
  fireEvent.keyDown(input, { key: 'Escape' });
  act(() => {
    vi.advanceTimersByTime(160);
  });
  expect(opener).toHaveFocus();
  key('k');
  fireEvent.keyDown(within(screen.getByRole('dialog')).getByRole('combobox'), {
    key: 'ArrowDown',
  });
  fireEvent.keyDown(within(screen.getByRole('dialog')).getByRole('combobox'), {
    key: 'ArrowUp',
  });
  fireEvent.keyDown(within(screen.getByRole('dialog')).getByRole('combobox'), {
    key: 'Enter',
  });
  act(() => {
    vi.advanceTimersByTime(160);
  });
  expect(
    screen.getAllByRole('button', { name: 'Activate database orders' }),
  ).toHaveLength(1);
});
it('search auto-expands matches, highlights literal characters, shows empty state and clears with Esc', () => {
  mount();
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  const views = sidebar.getByRole('button', { name: /^Views/ });
  expect(views).toHaveAttribute('aria-expanded', 'false');
  const input = screen.getByRole('textbox', { name: 'Search items' });
  fireEvent.change(input, { target: { value: 'summary' } });
  expect(views).toHaveAttribute('aria-expanded', 'true');
  expect(
    sidebar
      .getByRole('button', { name: 'order_summary' })
      .querySelectorAll('mark'),
  ).toHaveLength(7);
  expect(sidebar.queryByRole('button', { name: /order_items/ })).toBeNull();
  fireEvent.change(input, { target: { value: 'no_such_item' } });
  expect(sidebar.getByRole('status')).toHaveTextContent('No matches');
  fireEvent.keyDown(input, { key: 'Escape' });
  expect(input).toHaveValue('');
  expect(views).toHaveAttribute('aria-expanded', 'false');
  const tables = sidebar.getByRole('button', { name: /^Tables/ });
  tables.focus();
  fireEvent.keyDown(tables, { key: 'f', metaKey: true });
  expect(input).toHaveFocus();
  screen.getByRole('button', { name: 'Safe mode' }).focus();
  key('f');
  expect(input).not.toHaveFocus();
});
