// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { journaledTableEdits, openLogsFolder } from '@openrows/ipc';
import { CrashRecoverySheet } from './CrashRecoverySheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('names the journaled tables and their connection, and opens the logs folder', async () => {
  vi.mocked(journaledTableEdits).mockResolvedValue([
    {
      target: {
        profileId: 'id-orders-local',
        database: 'orders',
        schema: 'public',
        table: 'users',
      },
      rows: 2,
      savedAtMs: null,
    },
  ]);
  const user = userEvent.setup();
  render(<CrashRecoverySheet onClose={vi.fn()} />);
  expect(
    await screen.findByRole('list', { name: 'Tables with staged edits' }),
  ).toHaveTextContent('public.users on orders-local, 2 staged changes');
  await user.click(screen.getByRole('button', { name: 'Open logs folder' }));
  expect(openLogsFolder).toHaveBeenCalledTimes(1);
});

it('says so when no staged edits were left behind', async () => {
  vi.mocked(journaledTableEdits).mockResolvedValue([]);
  render(<CrashRecoverySheet onClose={vi.fn()} />);
  expect(
    await screen.findByText('No staged edits were left behind.'),
  ).toBeInTheDocument();
});
