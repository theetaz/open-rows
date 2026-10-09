// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  inspectObject,
  listExtensions,
  listRoles,
  roleGrants,
} from '@openrows/ipc';
import { InspectionTab } from './InspectionTab';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../table/structure/SqlViewer', () => ({
  SqlViewer: (props: { sql: string; label: string }) => (
    <pre aria-label={props.label}>{props.sql}</pre>
  ),
}));

it('shows account attributes, memberships and grants without edit controls', async () => {
  vi.mocked(listRoles).mockResolvedValue([
    {
      name: 'reader',
      scope: 'cluster',
      attributes: [{ name: 'Login', value: 'true' }],
      memberships: ['reporters'],
    },
  ]);
  vi.mocked(roleGrants).mockResolvedValue([
    {
      scope: 'table',
      object: 'public.items',
      privilege: 'SELECT',
      state: 'GRANT',
      grantor: 'owner',
    },
  ]);
  render(
    <ThemeProvider>
      <InspectionTab
        profileId="p"
        database="db"
        connected
        target={{ kind: 'role', name: 'reader', scope: 'cluster' }}
        onOpenSql={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(
    await screen.findByRole('table', { name: 'Grants' }),
  ).toHaveTextContent('public.items');
  expect(screen.getByText('Login')).toBeVisible();
  expect(screen.getByRole('table', { name: 'Member of' })).toHaveTextContent(
    'reporters',
  );
  expect(
    screen.getAllByRole('button').map((button) => button.textContent),
  ).toEqual(['Reload']);
  expect(screen.queryByRole('textbox')).toBeNull();
});

it('opens the exact stored definition in an editor and only reads once on open', async () => {
  const user = userEvent.setup();
  const onOpenSql = vi.fn();
  const sql = 'CREATE VIEW app.v AS SELECT 1;';
  vi.mocked(inspectObject).mockResolvedValue({
    attributes: [],
    sections: [
      { name: 'Columns', columns: ['Name', 'Type'], rows: [['id', 'integer']] },
    ],
    definition: sql,
  });
  const object = {
    schema: 'app',
    name: 'v',
    kind: 'view' as const,
    arguments: null,
  };
  render(
    <ThemeProvider>
      <InspectionTab
        profileId="p"
        database="db"
        connected
        target={{ kind: 'object', object }}
        onOpenSql={onOpenSql}
      />
    </ThemeProvider>,
  );
  expect(
    await screen.findByRole('table', { name: 'Columns' }),
  ).toHaveTextContent('integer');
  expect(inspectObject).toHaveBeenCalledOnce();
  expect(inspectObject).toHaveBeenCalledWith('p', 'db', object);
  await user.click(screen.getByRole('button', { name: 'Open in Editor' }));
  expect(onOpenSql).toHaveBeenCalledWith(sql);
  await user.click(screen.getByRole('button', { name: 'Reload' }));
  expect(
    await screen.findByLabelText('Object definition SQL'),
  ).toHaveTextContent(sql);
  expect(inspectObject).toHaveBeenCalledTimes(2);
});

it('shows installed and available extension versions with no edit actions', async () => {
  vi.mocked(listExtensions).mockResolvedValue([
    {
      name: 'hstore',
      version: '1.8',
      availableVersion: '1.9',
      schema: 'app',
      description: 'key/value data',
    },
  ]);
  render(
    <ThemeProvider>
      <InspectionTab
        profileId="p"
        database="db"
        connected
        target={{ kind: 'extension', name: 'hstore' }}
        onOpenSql={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(await screen.findByText('Installed version')).toBeVisible();
  expect(screen.getByText('1.8')).toBeVisible();
  expect(screen.getByText('1.9')).toBeVisible();
  expect(
    within(
      screen.getByRole('region', { name: 'Object inspection' }),
    ).getAllByRole('button'),
  ).toHaveLength(1);
});

it('does not query a disconnected server and surfaces permission failures', async () => {
  const target = { kind: 'role' as const, name: 'reader', scope: 'cluster' };
  const element = (connected: boolean) => (
    <ThemeProvider>
      <InspectionTab
        profileId="p"
        database="db"
        connected={connected}
        target={target}
        onOpenSql={vi.fn()}
      />
    </ThemeProvider>
  );
  const mounted = render(element(false));
  expect(listRoles).not.toHaveBeenCalled();
  vi.mocked(listRoles).mockRejectedValue(new Error('permission denied'));
  mounted.rerender(element(true));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'permission denied',
  );
});

it('keeps grants visible when direct memberships are not visible to the login', async () => {
  vi.mocked(listRoles).mockResolvedValue([
    {
      name: 'reader',
      scope: '%',
      memberships: [],
      attributes: [
        { name: 'Membership visibility', value: 'Not visible to this login' },
      ],
    },
  ]);
  vi.mocked(roleGrants).mockResolvedValue([
    {
      scope: 'global',
      object: '*.*',
      privilege: 'SELECT',
      state: 'GRANT',
      grantor: null,
    },
  ]);
  render(
    <ThemeProvider>
      <InspectionTab
        profileId="p"
        database="db"
        connected
        target={{ kind: 'role', name: 'reader', scope: '%' }}
        onOpenSql={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(
    await screen.findByRole('table', { name: 'Grants' }),
  ).toHaveTextContent('SELECT');
  expect(screen.getByRole('table', { name: 'Member of' })).toHaveTextContent(
    'Not visible to this login',
  );
  expect(screen.queryByRole('alert')).toBeNull();
});
