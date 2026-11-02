// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { EnvDto } from '@openrows/ipc';
import { profile, server } from '../../test/ipcStub';
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
        profile: undefined,
        status: undefined,
        database: '',
        schema: '',
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

function toolbarFor(env: EnvDto): HTMLElement {
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
      sidebar
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

it('no connection means no tint', () => {
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
  expect(
    screen
      .getByRole('banner', { name: 'Window toolbar' })
      .style.getPropertyValue('--env-color'),
  ).toBe('');
});
