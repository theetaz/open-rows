// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  createProfile,
  listProfiles,
  openConnection,
  parseConnectionUrl,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles, server } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';
import { useConnections } from './useConnections';
import { mountFlow, saveFlow, SheetsWithFlow } from './flow/flowTestKit';

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
      />
      <SheetsWithFlow connections={connections} />
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
    mongodb: null,
  });
  vi.mocked(createProfile).mockResolvedValueOnce(created);
  vi.mocked(testConnection).mockResolvedValueOnce({
    steps: [],
    server: server('17.2'),
    error: null,
    certificate: null,
    latency: null,
    elapsedMs: 12,
  });
  render(<Harness />);
  await user.tab();
  expect(screen.getByRole('button', { name: 'New connection' })).toHaveFocus();
  await user.keyboard('{Enter}');
  expect(
    await screen.findByRole('textbox', { name: 'Paste a connection URL' }),
  ).toHaveFocus();
  await user.keyboard('postgres://app@db.test:6543/orders');
  expect(
    await screen.findByText(/Detected PostgreSQL at db\.test:6543/),
  ).toBeVisible();
  // Return fills every field in and runs the test; Tab reaches Save.
  await user.keyboard('{Enter}');
  const save = await screen.findByRole('button', { name: 'Save' });
  for (let step = 0; step < 20 && document.activeElement !== save; step += 1)
    await user.tab();
  expect(save).toHaveFocus();
  await user.keyboard('{Enter}');
  await waitFor(() => {
    expect(createProfile).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'orders', host: 'db.test', port: 6543 }),
    );
  });
  // Saving closes the flow tab and selects the new connection, which connects.
  await waitFor(() => {
    expect(openConnection).toHaveBeenCalledWith('id-orders');
  });
  const tile = await screen.findByRole('button', { name: 'Switch to orders' });
  expect(screen.queryByRole('form')).toBeNull();
  // The flow is gone, so Tab starts at the rail again.
  await user.tab();
  expect(screen.getByRole('button', { name: 'New connection' })).toHaveFocus();
  await user.tab();
  expect(
    screen.getByRole('searchbox', { name: 'Search connections' }),
  ).toHaveFocus();
  await user.keyboard('{ArrowDown}');
  expect(tile).toHaveFocus();
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
  vi.mocked(updateProfile).mockResolvedValueOnce(asking);
  const { user } = await mountFlow({
    profile: profile('orders', { hasPassword: true }),
  });
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Password storage' }),
    'ask',
  );
  expect(
    screen.getByText(/saved password is removed from the Keychain on Save/),
  ).toBeVisible();
  expect(screen.getByLabelText('Password')).toHaveValue('');
  await saveFlow(user);
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
