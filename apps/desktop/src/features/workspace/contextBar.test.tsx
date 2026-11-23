// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { profile } from '../../test/ipcStub';
import { addJob } from '../jobs';
import { readSidebarObjects } from '../schema/sidebarStore';
import { tabFavourite } from './contextBar';
import { pathSegments } from './pathModel';
import { PathTools } from './PathTools';
import { WorkspaceToolbar } from './WorkspaceToolbar';

const handlers = {
  onPalette: vi.fn(),
  onSidebar: vi.fn(),
  onInspector: vi.fn(),
  onQuery: vi.fn(),
  onBack: null,
  onForward: null,
};

function tools(extra: Partial<Parameters<typeof PathTools>[0]> = {}): void {
  render(
    <PathTools
      health="connected"
      route="Direct"
      insecure={[]}
      safeMode={null}
      onSafeMode={vi.fn()}
      favourite={undefined}
      onRefresh={vi.fn()}
      {...extra}
    />,
  );
}

it('flags an insecure connection at the end of the path bar', () => {
  tools({ insecure: ['tlsDisabled'] });
  expect(document.body).toHaveTextContent('Not secure');
});

it('stars the shown tab object, including the exact routine overload', async () => {
  const user = userEvent.setup();
  const object = {
    database: 'db',
    schema: 'app',
    name: 'tax',
    kind: 'function' as const,
    arguments: 'numeric',
  };
  const favourite = tabFavourite(
    {
      id: 't1',
      connectionId: 'p',
      database: 'db',
      title: 'tax',
      kind: 'inspection',
      sql: '',
      inspection: { kind: 'object', object },
    },
    'p',
  );
  expect(favourite).toEqual({ ...object, profileId: 'p' });
  tools({ favourite });
  await user.click(screen.getByRole('button', { name: 'Add favourite' }));
  expect(readSidebarObjects('p', 'db').favourites).toEqual([
    { ...object, profileId: 'p' },
  ]);
  expect(
    screen.getByRole('button', { name: 'Remove favourite' }),
  ).toHaveAttribute('aria-pressed', 'true');
});

it('addresses a table tab as connection › database › schema › table', () => {
  const segments = pathSegments({
    tab: {
      id: 'table-1',
      kind: 'table',
      connectionId: 'p',
      database: 'orders',
      title: 'users',
      sql: '',
      table: { database: 'orders', schema: 'sales', name: 'users' },
    },
    profile: profile('reports'),
    database: 'orders',
    schema: 'public',
  });
  expect(segments.map((segment) => segment.text)).toEqual([
    'reports',
    'orders',
    'sales',
    'users',
  ]);
  expect(segments.at(-1)?.role).toBe('last');
});

it('stops short with "Choose a table…" and asks for a connection without one', () => {
  const short = pathSegments({
    tab: undefined,
    profile: profile('reports'),
    database: 'orders',
    schema: 'public',
  });
  expect(short.at(-1)).toMatchObject({ text: 'Choose a table…', role: 'todo' });
  const none = pathSegments({
    tab: undefined,
    profile: undefined,
    database: '',
    schema: '',
  });
  expect(none).toEqual([
    expect.objectContaining({ level: 'connection', role: 'prompt' }),
  ]);
});

it('without a connection the toolbar keeps the panel and history controls', () => {
  render(
    <WorkspaceToolbar
      {...handlers}
      path={null}
      connected={false}
      sidebar
      inspector={false}
    />,
  );
  expect(
    screen.getAllByRole('button').map((b) => b.getAttribute('aria-label')),
  ).toEqual([
    'Toggle sidebar (⌃⌘S)',
    'Back (⌃⌘←)',
    'Forward (⌃⌘→)',
    'Toggle inspector (⌥⌘I)',
  ]);
  expect(screen.getByRole('button', { name: 'Back (⌃⌘←)' })).toBeDisabled();
});

it('shows the jobs button while a job is listed', () => {
  const job = addJob({ kind: 'export', title: 'orders.csv' });
  render(
    <WorkspaceToolbar {...handlers} path={null} connected sidebar inspector />,
  );
  expect(
    screen.getByRole('button', { name: 'Background jobs (1 running)' }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: 'New SQL query' }),
  ).toHaveTextContent('Query');
  job.finish('Done');
});
