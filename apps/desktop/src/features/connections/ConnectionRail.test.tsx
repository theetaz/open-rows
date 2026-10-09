// SPDX-License-Identifier: Apache-2.0
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { searchProfiles } from '@openrows/ipc';
import { profile, server } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
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
    insecure: [],
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
    'analytics-prodprod⌘2PostgreSQL · db.example.test:5432Connected · PostgreSQL 16.4',
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
    screen.getByRole('searchbox', { name: 'Search connections' }),
  ).toHaveFocus();
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

it('opens the connection menu at the pointer on right-click', async () => {
  const user = userEvent.setup();
  const run = vi.fn();
  render(
    <ConnectionRail {...props} actionsFor={() => [['Disconnect', run]]} />,
  );
  const row = screen.getByRole('button', { name: 'Switch to analytics-prod' });
  fireEvent.contextMenu(row, { clientX: 120, clientY: 300 });
  const menu = screen.getByRole('menu', { name: 'analytics-prod actions' });
  expect(menu.parentElement?.style.top).toBe('300px');
  expect(screen.queryByRole('tooltip')).toBeNull();
  await user.click(within(menu).getByRole('menuitem', { name: 'Disconnect' }));
  expect(run).toHaveBeenCalledOnce();
  expect(screen.queryByRole('menu')).toBeNull();
});

it('keeps folded groups visible as tiles in the compact sidebar', async () => {
  const user = userEvent.setup();
  render(<ConnectionRail {...props} />);
  await user.click(screen.getByRole('button', { name: 'Collapse Staging' }));
  expect(screen.getAllByRole('button', { name: /^Switch to/ })).toHaveLength(2);
  await user.click(
    screen.getByRole('button', { name: 'Collapse workspace sidebar' }),
  );
  expect(screen.getAllByRole('button', { name: /^Switch to/ })).toHaveLength(3);
  await user.click(
    screen.getByRole('button', { name: 'Expand workspace sidebar' }),
  );
  await user.click(screen.getByRole('button', { name: 'Expand Staging' }));
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
        insecure: [],
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

it('marks insecure connections in the row and names the reasons in the tooltip', () => {
  const unverified = profile('reports', {
    host: 'db.example.test',
    insecure: ['tlsUnverified'],
  });
  const live: SessionStatus = {
    state: 'connected',
    insecure: ['hostKeyNotSaved'],
    server: server('16.4'),
    databases: [],
    generation: 1,
  };
  render(
    <ConnectionRail
      {...props}
      profiles={[unverified, profile('safe')]}
      statusOf={(id) =>
        id === unverified.id ? live : { state: 'disconnected' }
      }
    />,
  );
  const row = screen.getByRole('button', { name: 'Switch to reports' });
  expect(row.querySelector('[title]')).toHaveAttribute(
    'title',
    'The server certificate is not verified. The SSH host key was accepted once without saving it.',
  );
  const safe = screen.getByRole('button', { name: 'Switch to safe' });
  expect(safe.querySelector('[title]')).toBeNull();
  fireEvent.mouseEnter(row);
  expect(screen.getByRole('tooltip')).toHaveTextContent(
    'Not secure: The server certificate is not verified.',
  );
});

it('filters connections by the core search as you type, with keyboard navigation', async () => {
  const user = userEvent.setup();
  const onSelect = vi.fn();
  vi.mocked(searchProfiles).mockImplementation((query) =>
    Promise.resolve(
      profiles.filter((item) =>
        `${item.name} ${item.host} ${item.env.kind}`.includes(query),
      ),
    ),
  );
  render(<ConnectionRail {...props} onSelect={onSelect} />);
  const search = screen.getByRole('searchbox', { name: 'Search connections' });
  await user.type(search, 'prod');
  expect(searchProfiles).toHaveBeenLastCalledWith('prod');
  await waitFor(() => {
    expect(
      screen
        .getAllByRole('button', { name: /^Switch to/ })
        .map((tile) => tile.getAttribute('aria-label')),
    ).toEqual(['Switch to analytics-prod']);
  });
  await user.keyboard('{Enter}');
  expect(onSelect).toHaveBeenLastCalledWith('id-analytics-prod');
  await user.keyboard('{ArrowDown}');
  expect(
    screen.getByRole('button', { name: 'Switch to analytics-prod' }),
  ).toHaveFocus();
  search.focus();
  await user.keyboard('{Escape}');
  expect(search).toHaveValue('');
  expect(screen.getAllByRole('button', { name: /^Switch to/ })).toHaveLength(3);
  await user.type(search, 'nothing-matches');
  expect(await screen.findByRole('status')).toHaveTextContent(
    'No matching connections',
  );
});
