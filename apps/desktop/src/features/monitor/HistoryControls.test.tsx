// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import {
  configureMonitorHistory,
  monitorHistory,
  purgeMonitorHistory,
  writeSettingsFile,
} from '@openrows/ipc';
import { monitorSettings } from '../settings/monitorSettings';
import { HistoryControls } from './HistoryControls';
import { useMonitorHistory } from './useMonitorHistory';
import { monitorProfile } from './monitorFixture';
import { collectSettings, exportSettings } from '../settings/settingsFile';
vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
  configureMonitorHistory: vi.fn(() => Promise.resolve()),
  purgeMonitorHistory: vi.fn(() => Promise.resolve()),
  monitorHistory: vi.fn(() => Promise.resolve([])),
}));
beforeEach(() => {
  monitorSettings.set({ enabledProfiles: [] });
});
it('defaults off, enables per profile, persists the setting and disables with native purge', async () => {
  render(<HistoryControls profileId={monitorProfile.id} />);
  expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
  await userEvent.click(screen.getByRole('switch'));
  expect(await screen.findByText('Local history enabled.')).toBeInTheDocument();
  expect(configureMonitorHistory).toHaveBeenLastCalledWith([monitorProfile.id]);
  expect(writeSettingsFile).toHaveBeenCalledWith(
    expect.stringContaining(monitorProfile.id),
  );
  expect(collectSettings().monitoring).toEqual({
    enabledProfiles: [monitorProfile.id],
  });
  expect(JSON.parse(exportSettings()) as object).not.toHaveProperty(
    'monitoring',
  );
  await userEvent.click(screen.getByRole('switch'));
  expect(
    await screen.findByText('History disabled and purged.'),
  ).toBeInTheDocument();
  expect(configureMonitorHistory).toHaveBeenLastCalledWith([]);
});
it('purges one profile or all history without switching on collection', async () => {
  render(<HistoryControls profileId={monitorProfile.id} />);
  await userEvent.click(
    screen.getByRole('button', { name: 'Purge connection history' }),
  );
  expect(
    await screen.findByText('Connection history purged.'),
  ).toBeInTheDocument();
  expect(purgeMonitorHistory).toHaveBeenLastCalledWith(monitorProfile.id);
  await userEvent.click(
    screen.getByRole('button', { name: 'Purge all monitor history' }),
  );
  expect(
    await screen.findByText('All monitor history purged.'),
  ).toBeInTheDocument();
  expect(purgeMonitorHistory).toHaveBeenLastCalledWith(null);
  expect(configureMonitorHistory).not.toHaveBeenCalled();
});
function Range(): React.JSX.Element {
  const history = useMonitorHistory({
    profileId: monitorProfile.id,
    database: 'orders',
  });
  return (
    <>
      <button
        type="button"
        onClick={() => {
          history.setHours(1);
        }}
      >
        History range
      </button>
      <span>{history.enabled ? String(history.hours) : 'Off'}</span>
      <span>{String(history.points.length)}</span>
    </>
  );
}
it('never reads history while off and scopes enabled range reads to the profile and database', async () => {
  render(<Range />);
  await userEvent.click(screen.getByRole('button', { name: 'History range' }));
  expect(monitorHistory).not.toHaveBeenCalled();
  act(() => {
    monitorSettings.set({ enabledProfiles: [monitorProfile.id] });
  });
  await waitFor(() => {
    expect(monitorHistory).toHaveBeenCalledWith(
      { profileId: monitorProfile.id, database: 'orders' },
      2,
    );
  });
  act(() => {
    monitorSettings.set({ enabledProfiles: [] });
  });
  expect(screen.getByText('Off')).toBeInTheDocument();
});
