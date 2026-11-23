// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { EnvDto } from '@openrows/ipc';
import { profile, server } from '../../test/ipcStub';
import { addJob } from '../jobs';
import { readSidebarObjects } from '../schema/sidebarStore';
import { contextSegments, tabFavourite } from './contextBar';
import { WorkspaceToolbar } from './WorkspaceToolbar';

const handlers = {
  onDatabase: vi.fn(),
  onPalette: vi.fn(),
  onRefresh: vi.fn(),
  onSafeMode: vi.fn(),
  onSidebar: vi.fn(),
  onInspector: vi.fn(),
  onQuery: vi.fn(),
};

it('flags an insecure active connection in the window toolbar', () => {
  const context = contextSegments({
    profile: profile('reports', { insecure: ['tlsDisabled'] }),
    status: {
      state: 'connected',
      insecure: ['hostKeyNotSaved'],
      server: server('17.2'),
      databases: [],
      generation: 1,
    },
    database: 'orders',
    schema: 'public',
    objectPath: undefined,
  });
  expect(context.insecure).toEqual(['tlsDisabled', 'hostKeyNotSaved']);
  render(
    <WorkspaceToolbar
      {...handlers}
      context={context}
      health="connected"
      safeMode={null}
      sidebar
      inspector={false}
    />,
  );
  const bar = screen.getByLabelText('Connection context');
  expect(bar).toHaveTextContent('Not secure');
  expect(bar).toHaveTextContent(
    'TLS is off, so traffic to the server is readable.',
  );
  const safe = contextSegments({
    profile: profile('local'),
    status: undefined,
    database: '',
    schema: '',
    objectPath: undefined,
  });
  expect(safe.insecure).toEqual([]);
});

it('stars the active tab object, including the exact routine overload, from the context bar', async () => {
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
  render(
    <WorkspaceToolbar
      {...handlers}
      context={contextSegments({
        profile: profile('reports'),
        status: { state: 'disconnected' },
        database: 'db',
        schema: 'app',
        objectPath: undefined,
      })}
      favourite={favourite}
      health="connected"
      safeMode={null}
      sidebar
      inspector={false}
    />,
  );
  await user.click(screen.getByRole('button', { name: 'Add favourite' }));
  expect(readSidebarObjects('p', 'db').favourites).toEqual([
    { ...object, profileId: 'p' },
  ]);
  expect(
    screen.getByRole('button', { name: 'Remove favourite' }),
  ).toHaveAttribute('aria-pressed', 'true');
});

function toolbarFor(env: EnvDto, sidebar = true): HTMLElement {
  render(
    <WorkspaceToolbar
      {...handlers}
      context={contextSegments({
        profile: profile('reports', { env }),
        status: { state: 'disconnected' },
        database: 'orders',
        schema: 'public',
        objectPath: undefined,
      })}
      health="connected"
      safeMode={null}
      sidebar={sidebar}
      inspector={false}
    />,
  );
  return screen.getByRole('banner', { name: 'Window toolbar' });
}

it('colours the tag and the tint strip from the environment', () => {
  const toolbar = toolbarFor({ kind: 'prod', label: null, color: '#dc2626' });
  expect(toolbar.style.getPropertyValue('--env-color')).toBe(
    'var(--or-prod-fg)',
  );
  expect(screen.getByText('PROD')).toHaveAttribute('data-env', 'prod');
});

it('a custom tag keeps its label and uses its own colour', () => {
  const toolbar = toolbarFor({ kind: 'custom', label: 'qa', color: '#7c3aed' });
  expect(toolbar.style.getPropertyValue('--env-color')).toBe('#7c3aed');
  expect(screen.getByText('QA')).toHaveAttribute('data-env', 'custom');
});

it('local and dev connections get no strip, only the hairline', () => {
  const toolbar = toolbarFor({ kind: 'local', label: null, color: '#16a34a' });
  expect(toolbar.style.getPropertyValue('--env-color')).toBe('');
  expect(screen.getByText('LOCAL')).toHaveAttribute('data-env', 'local');
});

it('shows the address parts with `›` and keeps the engine out of the text', () => {
  toolbarFor({ kind: 'dev', label: null, color: '#16a34a' });
  const bar = screen.getByLabelText('Connection context');
  expect(bar).toHaveTextContent(/^DEV.*reports›orders›public$/);
  expect(bar).not.toHaveTextContent('Not connected');
  expect(bar.getAttribute('title')).toBe(
    'DEV | Not connected : reports : orders : public',
  );
  expect(
    screen.queryByRole('button', { name: /Open database \(/ }),
  ).toHaveTextContent('orders');
});

it('clears the traffic lights only while the sidebar is closed', () => {
  const toolbar = toolbarFor(
    { kind: 'dev', label: null, color: '#16a34a' },
    false,
  );
  expect(toolbar).toHaveAttribute('data-sidebar-state', 'closed');
});

it('no connection means no tint and only the two panel toggles', () => {
  render(
    <WorkspaceToolbar
      {...handlers}
      context={contextSegments({
        profile: undefined,
        status: undefined,
        database: '',
        schema: '',
        objectPath: undefined,
      })}
      health="disconnected"
      safeMode={null}
      sidebar
      inspector={false}
    />,
  );
  const toolbar = screen.getByRole('banner', { name: 'Window toolbar' });
  expect(toolbar.style.getPropertyValue('--env-color')).toBe('');
  expect(
    screen.getAllByRole('button').map((b) => b.getAttribute('aria-label')),
  ).toEqual(['Toggle sidebar (⌃⌘S)', 'Toggle inspector (⌥⌘I)']);
  expect(screen.queryByLabelText('Connection context')).toBeNull();
});

it('hides back, forward, the database button and an idle jobs button', () => {
  toolbarFor({ kind: 'dev', label: null, color: '#16a34a' });
  const names = screen
    .getAllByRole('button')
    .map((button) => button.getAttribute('aria-label'));
  expect(names).not.toContain('Back');
  expect(names).not.toContain('Forward');
  expect(names).not.toContain('Open database (⌘K)');
  expect(names).not.toContain('Background jobs');
  expect(screen.getByRole('button', { name: 'Safe mode' })).toBeDisabled();
});

it('shows the jobs button while a job is listed', () => {
  const job = addJob({ kind: 'export', title: 'orders.csv' });
  toolbarFor({ kind: 'dev', label: null, color: '#16a34a' });
  expect(
    screen.getByRole('button', { name: 'Background jobs (1 running)' }),
  ).toBeInTheDocument();
  job.finish('Done');
});
