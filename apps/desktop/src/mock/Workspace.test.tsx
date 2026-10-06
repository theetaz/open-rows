// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';

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
it('switches connections, context, schema, table list and forced prod safe mode', async () => {
  const user = mount();
  const safe = screen.getByRole('button', { name: 'Safe mode' });
  expect(safe).toHaveAttribute('aria-pressed', 'false');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'false');
  await user.click(
    screen.getByRole('button', { name: 'Switch to analytics-prod' }),
  );
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'analytics-prod › analytics › public › daily_revenue',
  );
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'PostgreSQL 15.8',
  );
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  await user.click(safe);
  expect(safe).toHaveAttribute('aria-pressed', 'true');
  expect(screen.getByRole('button', { name: /cohorts 18K/ })).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Switch to events' }));
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'ClickHouse 24.8',
  );
  expect(screen.getByRole('button', { name: /sessions 920K/ })).toBeVisible();
  await user.click(
    screen.getByRole('button', { name: 'Switch to app.sqlite' }),
  );
  expect(screen.getByLabelText('Connection context')).toHaveTextContent(
    'app.sqlite › main › main › users',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Schema' }),
    'temp',
  );
  expect(screen.getByLabelText('Connection context')).toHaveTextContent('temp');
});
it('switches sidebar segments, searches objects and expands all disclosure sections', async () => {
  const user = mount();
  const sidebar = within(
    screen.getByRole('complementary', { name: 'Sidebar' }),
  );
  for (const name of ['Views', 'Functions', 'Triggers', 'Sequences']) {
    const section = sidebar.getByRole('button', {
      name: new RegExp(`^${name}`),
    });
    expect(section).toHaveAttribute('aria-expanded', 'false');
    await user.click(section);
    expect(section).toHaveAttribute('aria-expanded', 'true');
  }
  await user.type(
    screen.getByRole('textbox', { name: 'Search items' }),
    'customers',
  );
  expect(sidebar.queryByRole('button', { name: /order_items/ })).toBeNull();
  expect(sidebar.getByRole('button', { name: /customers 412K/ })).toBeVisible();
  await user.clear(screen.getByRole('textbox', { name: 'Search items' }));
  await user.click(screen.getByRole('radio', { name: 'Queries' }));
  expect(sidebar.getByRole('button', { name: /Top customers/ })).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'History' }));
  expect(sidebar.getByRole('button', { name: /Yesterday/ })).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'Items' }));
  await user.click(sidebar.getByRole('button', { name: /customers 412K/ }));
  expect(screen.getByRole('tab', { name: /customers/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});
it('adds, switches and closes tabs with buttons and middle click', async () => {
  const user = mount();
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  expect(
    await screen.findByRole('textbox', { name: 'SQL editor' }),
  ).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Close SQL Query 1' }));
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  await user.click(screen.getByRole('tab', { name: /orders/ }));
  expect(screen.getByRole('grid')).toBeVisible();
  const tab = screen.getByRole('tab', { name: /customers/ });
  fireEvent(tab, new MouseEvent('auxclick', { button: 1, bubbles: true }));
  expect(screen.queryByRole('tab', { name: /customers/ })).toBeNull();
});
it('shows all five table views and handles filter removal, re-addition and a draft row', async () => {
  const user = mount();
  await user.click(screen.getByRole('radio', { name: 'Structure' }));
  expect(await screen.findByRole('table')).toHaveTextContent('numeric(12,2)');
  await user.click(screen.getByRole('radio', { name: 'Indexes' }));
  expect(await screen.findByRole('table')).toHaveTextContent(
    'orders_status_created_idx',
  );
  await user.click(screen.getByRole('radio', { name: 'Triggers' }));
  expect(screen.getByRole('heading', { name: 'Triggers' })).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'DDL' }));
  expect(
    (
      await screen.findAllByText('CREATE', {
        selector: '[data-token="keyword"]',
      })
    )[0],
  ).toBeVisible();
  await user.click(screen.getByRole('radio', { name: 'Data' }));
  await user.click(screen.getByRole('button', { name: 'Remove filter' }));
  expect(screen.queryByRole('button', { name: 'Remove filter' })).toBeNull();
  await user.click(screen.getByRole('button', { name: '+ Filter' }));
  expect(screen.getByRole('button', { name: 'Remove filter' })).toBeVisible();
  await user.click(screen.getByRole('button', { name: '+ Row' }));
  expect(screen.getByRole('grid')).toHaveAttribute('aria-rowcount', '42');
});
it('selects rows by pointer and bounded keyboard navigation and switches to Details', async () => {
  const user = mount();
  await user.click(screen.getByRole('row', { name: 'Order 10422' }));
  expect(screen.getByRole('radio', { name: 'Details' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  expect(
    await screen.findByRole('heading', { name: 'Order 10422' }),
  ).toBeVisible();
  const grid = screen.getByRole('grid');
  fireEvent.keyDown(grid, { key: 'ArrowDown' });
  expect(screen.getByRole('heading', { name: 'Order 10423' })).toBeVisible();
  fireEvent.keyDown(grid, { key: 'ArrowUp' });
  fireEvent.keyDown(grid, { key: 'Home' });
  fireEvent.keyDown(grid, { key: 'ArrowUp' });
  expect(screen.getByRole('row', { name: 'Order 10421' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  fireEvent.keyDown(grid, { key: 'End' });
  fireEvent.keyDown(grid, { key: 'ArrowDown' });
  expect(grid).toHaveAttribute('aria-activedescendant', 'order-row-39');
  fireEvent.keyDown(grid, { key: 'Home' });
  const edited = grid.querySelectorAll('[data-edited="true"]');
  expect(edited).toHaveLength(2);
  for (const cell of edited) {
    expect(cell).toHaveAccessibleName(/^Edited cell:/);
    expect(cell).not.toHaveTextContent('✎');
  }
  expect(
    screen.getByRole('row', { name: 'Deleted order 10427' }),
  ).toHaveTextContent('−');
});
