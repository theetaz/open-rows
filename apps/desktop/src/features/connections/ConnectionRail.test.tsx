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
  expect(
    within(rail).getAllByRole('button', { name: /^Switch to/ }),
  ).toHaveLength(3);
  for (const monogram of ['OS', 'AP', 'QB'])
    expect(rail).toHaveTextContent(monogram);
  const dots = Array.from(rail.querySelectorAll('i[data-status]')).map((dot) =>
    dot.getAttribute('data-status'),
  );
  expect(dots).toEqual(['none', 'connected', 'failed']);
  expect(rail.querySelectorAll('section[data-environment]')).toHaveLength(3);
  const selected = screen.getByRole('button', {
    name: 'Switch to orders-staging',
  });
  const other = screen.getByRole('button', {
    name: 'Switch to analytics-prod',
  });
  expect(selected).toHaveAttribute('aria-pressed', 'true');
  expect(selected).toHaveTextContent('orders-staging');
  fireEvent.mouseEnter(other);
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
  expect(screen.getByRole('button', { name: 'New connection' })).toHaveFocus();
  await user.tab();
  expect(
    screen.getByRole('button', { name: 'Collapse Staging' }),
  ).toHaveFocus();
  await user.tab();
  expect(tiles[0]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent('Not connected');
  await user.keyboard('{ArrowUp}');
  expect(tiles[2]).toHaveFocus();
  expect(screen.getByRole('tooltip')).toHaveTextContent('qa box');
  await user.keyboard('{ArrowDown}{Enter}');
  expect(props.onSelect).toHaveBeenCalledWith('id-orders-staging');
});

it('folds the entire sidebar while keeping database controls and remembering the choice', async () => {
  const user = userEvent.setup();
  const first = render(<ConnectionRail {...props} />);
  await user.click(
    screen.getByRole('button', { name: 'Collapse workspace sidebar' }),
  );
  expect(
    screen.getByRole('navigation', { name: 'Workspaces' }),
  ).toHaveAttribute('data-compact', 'true');
  expect(screen.getAllByRole('button', { name: /^Switch to/ })).toHaveLength(3);
  first.unmount();
  render(<ConnectionRail {...props} />);
  expect(
    screen.getByRole('button', { name: 'Expand workspace sidebar' }),
  ).toBeInTheDocument();
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

it('collapses environment and connection lists without closing sessions, and scopes equal database names', async () => {
  localStorage.clear();
  const user = userEvent.setup();
  const local = profile('local');
  const production = profile('production', {
    env: { kind: 'prod', label: null, color: '#dc2626' },
  });
  const onProfileDatabase = vi.fn();
  const onCloseProfileDatabase = vi.fn();
  const renderRail = () => (
    <ConnectionRail
      {...props}
      profiles={[local, production]}
      activeId={local.id}
      statusOf={() => ({
        state: 'connected',
        server: server('17'),
        databases: [{ name: 'orders', state: 'open', sessionId: 's' }],
        generation: 1,
      })}
      onProfileDatabase={onProfileDatabase}
      onCloseProfileDatabase={onCloseProfileDatabase}
    />
  );
  const rendered = render(renderRail());
  const databases = screen.getAllByRole('button', { name: 'Database orders' });
  await user.click(databases[1] as HTMLElement);
  expect(onProfileDatabase).toHaveBeenCalledWith(production.id, 'orders');
  await user.click(
    screen.getByRole('button', { name: 'Collapse production databases' }),
  );
  expect(
    screen.getAllByRole('button', { name: 'Database orders' }),
  ).toHaveLength(1);
  await user.click(screen.getByRole('button', { name: 'Collapse Local' }));
  expect(screen.queryByRole('button', { name: 'Switch to local' })).toBeNull();
  expect(onCloseProfileDatabase).not.toHaveBeenCalled();
  rendered.unmount();
  render(renderRail());
  expect(screen.getByRole('button', { name: 'Expand Local' })).toHaveAttribute(
    'aria-expanded',
    'false',
  );
  localStorage.clear();
});
