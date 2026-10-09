// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  listProfiles,
  openConnection,
  parseConnectionUrl,
  updateProfile,
} from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';
import { ConnectionSheets } from './ConnectionSheets';
import { useConnections } from './useConnections';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

function Harness(): React.JSX.Element {
  const connections = useConnections();
  return (
    <ThemeProvider>
      <ConnectionRail
        profiles={connections.profiles}
        activeId={connections.active?.id}
        statusOf={connections.statusOf}
        onSelect={connections.select}
        onNew={() => {
          connections.setSheet({ kind: 'new' });
        }}
        onSettings={undefined}
      />
      <ConnectionSheets connections={connections} />
    </ThemeProvider>
  );
}

const asking = profile('vault', { passwordStorage: 'ask', user: 'auditor' });

it('creates and connects a connection with the keyboard alone', async () => {
  const user = userEvent.setup();
  const created = profile('orders', { host: 'db.test', port: 6543 });
  vi.mocked(listProfiles)
    .mockResolvedValueOnce({ profiles: [], unreadable: [] })
    .mockResolvedValueOnce({ profiles: [created], unreadable: [] });
  vi.mocked(parseConnectionUrl).mockResolvedValue({
    engine: 'postgres',
    file: null,
    host: 'db.test',
    port: 6543,
    database: 'orders',
    user: 'app',
    tlsMode: 'prefer',
    passwordProvided: false,
  });
  vi.mocked(createProfile).mockResolvedValueOnce(created);
  render(<Harness />);
  await user.tab();
  expect(screen.getByRole('button', { name: 'New connection' })).toHaveFocus();
  await user.keyboard('{Enter}');
  expect(
    screen.getByRole('searchbox', { name: /connection types/ }),
  ).toHaveFocus();
  await user.keyboard('{Enter}');
  expect(screen.getByRole('textbox', { name: 'URL' })).toHaveFocus();
  await user.keyboard('postgres://app@db.test:6543/orders');
  expect(await screen.findByDisplayValue('db.test')).toBeVisible();
  await user.keyboard('{Enter}');
  await waitFor(() => {
    expect(createProfile).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'orders', host: 'db.test', port: 6543 }),
    );
  });
  const tile = await screen.findByRole('button', { name: 'Switch to orders' });
  await waitFor(() => {
    expect(screen.queryByRole('dialog')).toBeNull();
  });
  // The form's opener (the type picker) is gone, so Tab starts at the rail.
  await user.tab();
  expect(screen.getByRole('button', { name: 'New connection' })).toHaveFocus();
  await user.tab();
  expect(
    screen.getByRole('searchbox', { name: 'Search connections' }),
  ).toHaveFocus();
  await user.keyboard('{ArrowDown}');
  expect(tile).toHaveFocus();
  await user.keyboard('{Enter}');
  await waitFor(() => {
    expect(openConnection).toHaveBeenCalledWith('id-orders');
  });
});

it('asks for the password of an "ask every time" connection and never keeps it', async () => {
  const user = userEvent.setup();
  vi.mocked(listProfiles).mockResolvedValue({
    profiles: [...sampleProfiles, asking],
    unreadable: [],
  });
  vi.mocked(openConnection).mockClear();
  render(<Harness />);
  await user.click(
    await screen.findByRole('button', { name: 'Switch to vault' }),
  );
  const sheet = screen.getByRole('dialog', { name: 'Connect to vault' });
  expect(sheet).toHaveTextContent('auditor@localhost:5432');
  expect(screen.getByLabelText('Password')).toHaveFocus();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(openConnection).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Switch to vault' }));
  await user.keyboard('pw-asked-synthetic{Enter}');
  await waitFor(() => {
    expect(openConnection).toHaveBeenCalledWith(
      'id-vault',
      'pw-asked-synthetic',
    );
  });
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(document.body.innerHTML).not.toContain('pw-asked-synthetic');
  await user.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
  expect(openConnection).toHaveBeenLastCalledWith('id-orders-local');
});

it('switches a saved connection to ask every time', async () => {
  const user = userEvent.setup();
  const { ConnectionSheet } = await import('./ConnectionSheet');
  vi.mocked(updateProfile).mockResolvedValueOnce(asking);
  render(
    <ThemeProvider>
      <ConnectionSheet
        profile={profile('orders', { hasPassword: true })}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Password storage' }),
    'ask',
  );
  expect(
    screen.getByText(/saved password is removed from the Keychain on Save/),
  ).toBeVisible();
  expect(screen.getByLabelText('Password')).toHaveValue('');
  await user.click(screen.getByRole('button', { name: 'Save' }));
  expect(updateProfile).toHaveBeenCalledWith(
    'id-orders',
    expect.objectContaining({ passwordStorage: 'ask' }),
  );
});

it.each(['none', 'command'] as const)(
  'connects with %s without a password sheet',
  async (passwordStorage) => {
    const user = userEvent.setup();
    vi.mocked(listProfiles).mockResolvedValue({
      profiles: [profile('policy', { passwordStorage })],
      unreadable: [],
    });
    render(<Harness />);
    await user.click(
      await screen.findByRole('button', { name: 'Switch to policy' }),
    );
    await waitFor(() => {
      expect(openConnection).toHaveBeenCalledWith('id-policy');
    });
    expect(screen.queryByRole('dialog')).toBeNull();
  },
);
