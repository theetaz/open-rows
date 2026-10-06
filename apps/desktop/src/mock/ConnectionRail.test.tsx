// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ConnectionRail } from './ConnectionRail';
const props = { selected: 0, onSelect: vi.fn(), onGallery: vi.fn() };
it('contains only workspace tiles and bottom actions, with selected and hover indicators', () => {
  render(<ConnectionRail {...props} />);
  const rail = screen.getByRole('navigation', { name: 'Workspaces' });
  expect(within(rail).getAllByRole('button')).toHaveLength(7);
  expect(rail).toHaveTextContent('OSAPEVASAPI');
  expect(rail.querySelectorAll('small')).toHaveLength(0);
  expect(rail.querySelectorAll('[data-environment]')).toHaveLength(4);
  const selected = screen.getByRole('button', {
    name: 'Switch to orders-staging',
  });
  const other = screen.getByRole('button', {
    name: 'Switch to analytics-prod',
  });
  expect(selected.querySelector('[data-state]')).toHaveAttribute(
    'data-state',
    'selected',
  );
  expect(other.querySelector('[data-state]')).toHaveAttribute(
    'data-state',
    'idle',
  );
  fireEvent.mouseEnter(other);
  expect(other.querySelector('[data-state]')).toHaveAttribute(
    'data-state',
    'hover',
  );
  expect(screen.getByRole('tooltip')).toHaveTextContent(
    'analytics-prodprodPostgreSQL 15.8 · db.example.test',
  );
  expect(other).toHaveAccessibleDescription(/analytics-prod.*prod.*PostgreSQL/);
  fireEvent.mouseLeave(other);
  expect(screen.queryByRole('tooltip')).toBeNull();
  fireEvent.click(other);
  expect(props.onSelect).toHaveBeenCalledWith(1);
});
it('supports Tab and wrapping arrows with focus tooltips without changing selection', async () => {
  const user = userEvent.setup();
  render(<ConnectionRail {...props} />);
  const tiles = screen.getAllByRole('button', { name: /^Switch to/ });
  await user.tab();
  expect(tiles[0]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent('orders-staging');
  await user.keyboard('{ArrowUp}');
  expect(tiles[3]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent(
    'app.sqlitelocalSQLite 3.46 · local file',
  );
  await user.keyboard('{ArrowDown}');
  expect(tiles[0]).toHaveFocus();
  await user.tab();
  expect(tiles[1]).toHaveFocus();
  await user.keyboard('{Enter}');
  expect(props.onSelect).toHaveBeenCalledWith(1);
});
