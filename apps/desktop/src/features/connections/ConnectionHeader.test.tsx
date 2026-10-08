// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  closeConnection,
  confirmDestructive,
  deleteProfile,
  duplicateProfile,
  listProfiles,
  openConnection,
} from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles } from '../../test/ipcStub';
import { ConnectionHeader } from './ConnectionHeader';
import { ConnectionRail } from './ConnectionRail';
import { ConnectionSheets } from './ConnectionSheets';
import { connectionActions } from './connectionActions';
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
        actionsFor={(item) => connectionActions(item, connections)}
      />
      <ConnectionHeader connections={connections} />
      <ConnectionSheets connections={connections} />
    </ThemeProvider>
  );
}

async function menu(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
  connection = 'orders-local',
) {
  await user.click(
    await screen.findByRole('button', { name: `${connection} actions` }),
  );
  await user.click(screen.getByRole('menuitem', { name }));
}

it('connects on selection, shows failures and disconnects from the row menu', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  expect(await screen.findAllByText('Not connected')).not.toHaveLength(0);
  vi.mocked(openConnection).mockRejectedValueOnce(
    new Error('network error: localhost:5432: connection refused'),
  );
  await user.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'connection refused',
  );
  await menu(user, 'Connect');
  expect(
    await screen.findByText('Connected · PostgreSQL 17.2'),
  ).toBeInTheDocument();
  expect(screen.queryByRole('alert')).toBeNull();
  await menu(user, 'Disconnect');
  expect(closeConnection).toHaveBeenCalledWith('id-orders-local');
  await waitFor(() => {
    expect(screen.queryByText('Connected · PostgreSQL 17.2')).toBeNull();
  });
});

it('duplicates and deletes only after confirmation', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(
    await screen.findByRole('button', { name: 'Switch to analytics-prod' }),
  );
  const copy = profile('analytics-prod copy');
  vi.mocked(duplicateProfile).mockResolvedValueOnce(copy);
  vi.mocked(listProfiles).mockResolvedValueOnce({
    profiles: [...sampleProfiles, copy],
    unreadable: [],
  });
  await menu(user, 'Duplicate', 'analytics-prod');
  expect(duplicateProfile).toHaveBeenCalledWith('id-analytics-prod');
  const copyName = 'analytics-prod copy';
  expect(
    await screen.findByRole('button', { name: `Switch to ${copyName}` }),
  ).toBeVisible();
  vi.mocked(confirmDestructive).mockResolvedValueOnce(false);
  await menu(user, 'Delete…', copyName);
  expect(confirmDestructive).toHaveBeenLastCalledWith(
    expect.objectContaining({
      title: 'Delete “analytics-prod copy”?',
      action: 'Delete',
    }),
  );
  expect(deleteProfile).not.toHaveBeenCalled();
  await menu(user, 'Delete…', copyName);
  const dialog = await screen.findByRole('dialog', {
    name: 'Delete “analytics-prod copy”?',
  });
  expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
  await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  expect(deleteProfile).not.toHaveBeenCalled();
  vi.mocked(confirmDestructive).mockResolvedValueOnce(true);
  vi.mocked(listProfiles).mockResolvedValueOnce({
    profiles: [...sampleProfiles],
    unreadable: [],
  });
  await menu(user, 'Delete…', copyName);
  await waitFor(() => {
    expect(deleteProfile).toHaveBeenCalledWith('id-analytics-prod copy');
  });
  await waitFor(() => {
    expect(
      screen.queryByRole('button', { name: `Switch to ${copyName}` }),
    ).toBeNull();
  });
});

it('opens the new connection sheet from the rail', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('button', { name: 'New connection' }));
  expect(screen.getByRole('dialog', { name: 'New connection' })).toBeVisible();
});
