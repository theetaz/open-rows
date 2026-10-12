// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { shareProjectConnections } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { ProjectConnections } from './ProjectConnections';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('shares only selected definitions after the explicit save action', async () => {
  const user = userEvent.setup();
  render(
    <ProjectConnections
      project={{
        root: '/work/p',
        name: 'p',
        defaultConnection: null,
        queryFolders: ['queries'],
        snippetFolders: [],
        connectionsFile: null,
        entries: [],
        truncated: false,
      }}
      profiles={[profile('team'), profile('private')]}
    />,
  );
  await user.click(screen.getByRole('button', { name: 'Share connections…' }));
  await user.click(screen.getByRole('checkbox', { name: 'team' }));
  expect(shareProjectConnections).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Save definitions' }));
  await waitFor(() => {
    expect(shareProjectConnections).toHaveBeenCalledWith('/work/p', [
      'id-team',
    ]);
  });
});
