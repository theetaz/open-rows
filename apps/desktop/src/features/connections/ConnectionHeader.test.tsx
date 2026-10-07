// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import {
  closeSession,
  deleteProfile,
  duplicateProfile,
  listProfiles,
  openSession,
} from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles } from '../../test/ipcStub';
import { ConnectionHeader } from './ConnectionHeader';
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
        onGallery={vi.fn()}
      />
      <ConnectionHeader connections={connections} />
      <ConnectionSheets connections={connections} />
    </ThemeProvider>
  );
}

async function menu(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByRole('button', { name: 'Connection menu' }));
  await user.click(screen.getByRole('menuitem', { name }));
}

it('connects on selection, shows failures and disconnects from the menu', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  expect(
    await screen.findByText('Choose a connection in the rail.'),
  ).toBeVisible();
  vi.mocked(openSession).mockRejectedValueOnce(
    new Error('network error: localhost:5432: connection refused'),
  );
  await user.click(
    screen.getByRole('button', { name: 'Switch to orders-local' }),
  );
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'connection refused',
  );
  await menu(user, 'Connect');
  expect(await screen.findByText('Connected · PostgreSQL 17.2')).toBeVisible();
  await menu(user, 'Disconnect');
  expect(closeSession).toHaveBeenCalledWith('session-id-orders-local');
  expect(await screen.findByText('Not connected')).toBeVisible();
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
  await menu(user, 'Duplicate');
  expect(duplicateProfile).toHaveBeenCalledWith('id-analytics-prod');
  expect(await screen.findByTitle('analytics-prod copy')).toBeVisible();
  await menu(user, 'Delete…');
  const dialog = screen.getByRole('dialog', { name: 'Delete connection' });
  await user.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  expect(deleteProfile).not.toHaveBeenCalled();
  await menu(user, 'Delete…');
  vi.mocked(listProfiles).mockResolvedValueOnce({
    profiles: [...sampleProfiles],
    unreadable: [],
  });
  await user.click(screen.getByRole('button', { name: 'Delete connection' }));
  expect(deleteProfile).toHaveBeenCalledWith('id-analytics-prod copy');
  expect(
    await screen.findByText('Choose a connection in the rail.'),
  ).toBeVisible();
});

it('opens the new connection sheet from the rail', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('button', { name: 'New connection' }));
  expect(screen.getByRole('dialog', { name: 'New connection' })).toBeVisible();
});
