// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  exportConnections,
  importConnections,
  previewConnections,
  cancelConnectionImport,
} from '@openrows/ipc';
import type { ImportPreview } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import ConnectionTransferSheet from './ConnectionTransferSheet';
vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
const preview: ImportPreview = {
  token: 'preview',
  warnings: ['include directive ignored'],
  rows: [
    { profile: profile('new'), conflict: 'new', hasPassword: true },
    { profile: profile('conflict'), conflict: 'sameName', hasPassword: false },
    { profile: profile('same'), conflict: 'identical', hasPassword: false },
  ],
};
it('writes nothing until confirmation and requires password save opt-in', async () => {
  const user = userEvent.setup();
  vi.mocked(previewConnections).mockResolvedValue(preview);
  const close = vi.fn();
  const imported = vi.fn();
  render(
    <ThemeProvider>
      <ConnectionTransferSheet
        mode="import"
        profiles={[]}
        onClose={close}
        onImported={imported}
      />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Preview…' }));
  expect(
    await screen.findByText('Identical (skip)', { exact: false }),
  ).toBeVisible();
  expect(importConnections).not.toHaveBeenCalled();
  const password = screen.getByRole('checkbox', {
    name: 'Save password for new in keychain',
  });
  expect(password).not.toBeChecked();
  await user.click(password);
  await user.click(screen.getByRole('button', { name: 'Confirm import' }));
  await waitFor(() => {
    expect(importConnections).toHaveBeenCalledWith('preview', [
      { choice: 'keepBoth', savePassword: true },
      { choice: 'skip', savePassword: false },
      { choice: 'skip', savePassword: false },
    ]);
  });
  expect(imported).toHaveBeenCalledOnce();
  expect(close).toHaveBeenCalledOnce();
});
it('cancel and unmount discard the preview without importing', async () => {
  const user = userEvent.setup();
  vi.mocked(previewConnections).mockResolvedValue(preview);
  const view = render(
    <ThemeProvider>
      <ConnectionTransferSheet
        mode="import"
        profiles={[]}
        onClose={vi.fn()}
        onImported={vi.fn()}
      />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Preview…' }));
  await screen.findByText('include directive ignored');
  view.unmount();
  expect(cancelConnectionImport).toHaveBeenCalledWith('preview');
  expect(importConnections).not.toHaveBeenCalled();
});
it('exports a selected group and then selected profiles', async () => {
  const user = userEvent.setup();
  const profiles = [
    profile('a', { group: 'team' }),
    profile('b', { group: 'team' }),
    profile('c'),
  ];
  render(
    <ThemeProvider>
      <ConnectionTransferSheet
        mode="export"
        profiles={profiles}
        onClose={vi.fn()}
        onImported={vi.fn()}
      />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Select group: team' }));
  expect(screen.getByRole('checkbox', { name: 'c' })).not.toBeChecked();
  await user.click(screen.getByRole('button', { name: 'Export…' }));
  expect(exportConnections).toHaveBeenCalledWith(['id-a', 'id-b']);
});

it('previews a project definitions file without the native file picker or automatic import', async () => {
  const user = userEvent.setup();
  vi.mocked(previewConnections).mockResolvedValue(preview);
  render(
    <ThemeProvider>
      <ConnectionTransferSheet
        mode="import"
        profiles={[]}
        projectRoot="/work/team"
        onClose={vi.fn()}
        onImported={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(
    screen.queryByRole('combobox', { name: 'Source' }),
  ).not.toBeInTheDocument();
  expect(previewConnections).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Preview…' }));
  await waitFor(() => {
    expect(previewConnections).toHaveBeenCalledWith({
      project: { root: '/work/team' },
    });
  });
  expect(importConnections).not.toHaveBeenCalled();
});
