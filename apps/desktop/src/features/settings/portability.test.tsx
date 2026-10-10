// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { writeSettingsFile } from '@openrows/ipc';
import { exportSettings, importSettings, resetSettings } from './settingsFile';
import { parseGeneral, generalSettings, updateGeneral } from './general';
import { SettingsSheet } from './SettingsSheet';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('exports only preferences even with passwords, history and snippets in the device profile', () => {
  localStorage.setItem(
    'openrows.profiles',
    JSON.stringify([{ password: 'pw', history: ['SELECT private'] }]),
  );
  localStorage.setItem(
    'openrows.snippets',
    JSON.stringify({
      items: [{ id: 's', name: 'secret', prefix: 's', body: 'pw' }],
    }),
  );
  localStorage.setItem(
    'openrows.history',
    JSON.stringify({ entries: ['private-history'] }),
  );
  localStorage.setItem(
    'openrows.general',
    JSON.stringify({ password: 'pw', restoreSession: false }),
  );
  const text = exportSettings();
  expect(text).not.toMatch(
    /password|private-history|SELECT private|"history"|"snippets"/,
  );
  expect(JSON.parse(text)).toMatchObject({
    openrows: 1,
    general: { restoreSession: false },
  });
});

it('validates portable JSON before importing and rolls back a failed file write', async () => {
  await importSettings(
    JSON.stringify({
      openrows: 1,
      general: {
        updateChannel: 'beta',
        newTab: 'connection',
        updateNotifications: false,
      },
    }),
  );
  expect(generalSettings.get()).toMatchObject({
    updateChannel: 'beta',
    newTab: 'connection',
    updateNotifications: false,
  });
  for (const settings of [
    { password: 'pw' },
    { general: { password: 'pw' } },
    { general: { updateChannel: 'unsafe' } },
    { history: {} },
    {
      appearance: {
        customThemes: [
          {
            id: 'custom-x',
            name: 'x',
            light: { canvas: '#ffffff', accent: '#ffffff' },
            dark: { canvas: '#000000', accent: '#ffffff' },
          },
        ],
      },
    },
  ])
    await expect(
      importSettings(JSON.stringify({ openrows: 1, ...settings })),
    ).rejects.toThrow();
  vi.mocked(writeSettingsFile).mockRejectedValueOnce(new Error('disk full'));
  await expect(
    importSettings('{"openrows":1,"general":{"updateChannel":"stable"}}'),
  ).rejects.toThrow('disk full');
  expect(generalSettings.get().updateChannel).toBe('beta');
});

it('resets one group or all preferences without deleting profiles or history entries', () => {
  localStorage.setItem('openrows.profiles', 'profile');
  localStorage.setItem('openrows.history.entries', 'history');
  updateGeneral({ newTab: 'connection', updateChannel: 'beta' });
  localStorage.setItem('openrows.editor', '{"wrap":true}');
  resetSettings('general');
  expect(generalSettings.get().updateChannel).toBe('stable');
  expect(localStorage.getItem('openrows.editor')).toBe('{"wrap":true}');
  resetSettings();
  expect(localStorage.getItem('openrows.profiles')).toBe('profile');
  expect(localStorage.getItem('openrows.history.entries')).toBe('history');
  expect(
    parseGeneral({
      newTab: 'invalid',
      updateChannel: 'nightly',
      updateNotifications: 3,
    }),
  ).toMatchObject({
    newTab: 'query',
    updateChannel: 'stable',
    updateNotifications: true,
  });
});

it('highlights search matches and requires confirmation for group/global resets', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <SettingsSheet onClose={vi.fn()} />
    </ThemeProvider>,
  );
  await user.type(screen.getByLabelText('Search settings'), 'beta');
  expect(screen.getByText('beta', { selector: 'mark' })).toBeVisible();
  await user.clear(screen.getByLabelText('Search settings'));
  await user.selectOptions(screen.getByLabelText('Update channel'), 'beta');
  await user.click(screen.getByRole('button', { name: 'Reset group' }));
  expect(generalSettings.get().updateChannel).toBe('beta');
  await user.click(
    within(
      screen.getByRole('dialog', { name: 'Reset to defaults?' }),
    ).getByRole('button', { name: 'Reset defaults' }),
  );
  expect(generalSettings.get().updateChannel).toBe('stable');
  await user.selectOptions(screen.getByLabelText('Update channel'), 'beta');
  await user.click(screen.getByRole('button', { name: 'Reset all settings' }));
  await user.click(
    within(
      screen.getByRole('dialog', { name: 'Reset to defaults?' }),
    ).getByRole('button', { name: 'Cancel' }),
  );
  expect(generalSettings.get().updateChannel).toBe('beta');
  await user.click(screen.getByRole('button', { name: 'Reset all settings' }));
  await user.click(
    within(
      screen.getByRole('dialog', { name: 'Reset to defaults?' }),
    ).getByRole('button', { name: 'Reset defaults' }),
  );
  expect(generalSettings.get().updateChannel).toBe('stable');
  await user.click(screen.getByRole('button', { name: 'Import settings' }));
  fireEvent.change(screen.getByLabelText('Portable settings JSON'), {
    target: { value: '{"openrows":1,"credentials":{}}' },
  });
  await user.click(screen.getByRole('button', { name: 'Import preferences' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'credentials: unknown key',
  );
});

it('selects different signed HTTPS manifests and refuses unconfigured or credential-bearing feeds', async () => {
  const { updateFeed } = await import('./updateFeed');
  const config = {
    stable: 'https://releases.example.invalid/stable.json',
    beta: 'https://releases.example.invalid/beta.json',
    publicKey: 'test-only',
  };
  expect(updateFeed('stable', config)).toContain('/stable.json');
  expect(updateFeed('beta', config)).toContain('/beta.json');
  expect(updateFeed('stable', { ...config, publicKey: null })).toBeNull();
  expect(
    updateFeed('stable', { ...config, stable: 'http://example.invalid/feed' }),
  ).toBeNull();
  expect(
    updateFeed('stable', {
      ...config,
      stable: 'https://app:pw@example.invalid/feed',
    }),
  ).toBeNull();
});

it('requests explicit history-copy consent and schedules only after confirmation', async () => {
  const { chooseDataLocation, scheduleDataLocation } =
    await import('@openrows/ipc');
  vi.mocked(chooseDataLocation).mockResolvedValue('/empty-local-folder');
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <SettingsSheet initial="locations" onClose={vi.fn()} />
    </ThemeProvider>,
  );
  await user.click(screen.getByRole('button', { name: 'Choose data folder' }));
  const consent = await screen.findByRole('dialog', {
    name: 'Copy local data on restart?',
  });
  expect(consent).toHaveTextContent('local history');
  expect(consent).toHaveTextContent('Keychain secrets');
  await user.click(within(consent).getByRole('button', { name: 'Cancel' }));
  expect(scheduleDataLocation).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Choose data folder' }));
  await user.click(
    await screen.findByRole('button', { name: 'Copy on next restart' }),
  );
  expect(scheduleDataLocation).toHaveBeenCalledWith('/empty-local-folder');
  expect(
    await screen.findByText(/Restart OpenRows to copy data/),
  ).toBeVisible();
});
