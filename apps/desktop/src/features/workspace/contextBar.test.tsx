// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { profile, server } from '../../test/ipcStub';
import { contextSegments } from './contextBar';
import { WorkspaceToolbar } from './WorkspaceToolbar';

const handlers = {
  onDatabase: vi.fn(),
  onPalette: vi.fn(),
  onRefresh: vi.fn(),
  onSafe: vi.fn(),
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
      safe={false}
      prod={false}
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
