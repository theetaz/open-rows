// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { profile, server } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';
import type { SessionStatus } from './sessions';

const profiles = [
  profile('orders-staging', {
    env: { kind: 'staging', label: null, color: '#d97706' },
  }),
  profile('analytics-prod', {
    host: 'db.example.test',
    env: { kind: 'prod', label: null, color: '#dc2626' },
  }),
  profile('qa box', {
    env: { kind: 'custom', label: 'qa', color: '#123456' },
  }),
];
const statuses: Record<string, SessionStatus> = {
  'id-analytics-prod': {
    state: 'connected',
    server: server('16.4'),
    databases: [],
    generation: 1,
  },
  'id-qa box': { state: 'failed', message: 'refused' },
};
const props = {
  profiles,
  activeId: 'id-orders-staging',
  statusOf: (id: string): SessionStatus =>
    statuses[id] ?? { state: 'disconnected' },
  onSelect: vi.fn(),
  onNew: vi.fn(),
  onGallery: vi.fn(),
};

it('shows saved connections as tiles with connection-status dots, status tooltips and add', () => {
  render(<ConnectionRail {...props} />);
  const rail = screen.getByRole('navigation', { name: 'Workspaces' });
  expect(within(rail).getAllByRole('button')).toHaveLength(6);
  expect(rail).toHaveTextContent('OSAPQB');
  const dots = Array.from(rail.querySelectorAll('i[data-status]')).map((dot) =>
    dot.getAttribute('data-status'),
  );
  expect(dots).toEqual(['none', 'connected', 'failed']);
  expect(rail.querySelector('[data-environment]')).toBeNull();
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
  fireEvent.mouseEnter(other);
  expect(other.querySelector('[data-state]')).toHaveAttribute(
    'data-state',
    'hover',
  );
  expect(screen.getByRole('tooltip')).toHaveTextContent(
    'analytics-prodprodPostgreSQL · db.example.test:5432Connected · PostgreSQL 16.4',
  );
  fireEvent.mouseLeave(other);
  expect(screen.queryByRole('tooltip')).toBeNull();
  fireEvent.click(other);
  expect(props.onSelect).toHaveBeenCalledWith('id-analytics-prod');
  fireEvent.click(screen.getByRole('button', { name: 'New connection' }));
  expect(props.onNew).toHaveBeenCalledOnce();
});

it('supports Tab and wrapping arrows with focus tooltips', async () => {
  const user = userEvent.setup();
  render(<ConnectionRail {...props} />);
  const tiles = screen.getAllByRole('button', { name: /^Switch to/ });
  await user.tab();
  expect(tiles[0]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent('Not connected');
  await user.keyboard('{ArrowUp}');
  expect(tiles[2]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent('qa box');
  await user.keyboard('{ArrowDown}{Enter}');
  expect(props.onSelect).toHaveBeenCalledWith('id-orders-staging');
});

it('lists the active connection opened databases as tiles with switch and close', async () => {
  const user = userEvent.setup();
  const rail = {
    ...props,
    databases: [
      { name: 'orders', state: 'open', sessionId: 's1' },
      { name: 'a_very_long_reporting_database', state: 'opening' },
      { name: 'broken', state: 'failed', message: 'does not exist' },
    ] as const,
    database: 'orders',
    onDatabase: vi.fn(),
    onCloseDatabase: vi.fn(),
  };
  render(<ConnectionRail {...rail} />);
  const group = screen.getByRole('group', { name: 'Opened databases' });
  const tiles = within(group).getAllByRole('button');
  expect(tiles.map((tile) => tile.getAttribute('aria-pressed'))).toEqual([
    'true',
    'false',
    'false',
  ]);
  expect(tiles[1]).toHaveAttribute(
    'title',
    'a_very_long_reporting_database — opening…',
  );
  expect(tiles[2]).toHaveAttribute('title', 'broken — does not exist');
  await user.click(tiles[1] as HTMLElement);
  expect(rail.onDatabase).toHaveBeenCalledWith(
    'a_very_long_reporting_database',
  );
  fireEvent.contextMenu(tiles[0] as HTMLElement);
  await user.click(screen.getByRole('menuitem', { name: 'Close database' }));
  expect(rail.onCloseDatabase).toHaveBeenCalledWith('orders');
  expect(tiles[0]).toHaveFocus();
});
