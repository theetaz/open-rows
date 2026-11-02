// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { act } from 'react';
import { checkForUpdate } from '@openrows/ipc';
import { generalSettings, updateGeneral } from './general';
import { checkUpdate, dismissUpdate, useUpdate } from './updater';

const close = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
vi.mock('@openrows/ipc', () => ({ checkForUpdate: vi.fn() }));
vi.mock('./updateFeed', () => ({
  updateFeed: () => 'https://example.invalid/stable.json',
}));
vi.mock('@tauri-apps/plugin-updater', () => ({
  Update: class {
    version: string;
    close = close;
    constructor(metadata: { version: string }) {
      this.version = metadata.version;
    }
  },
}));

beforeEach(() => {
  dismissUpdate();
  vi.clearAllMocks();
  updateGeneral({ updateChannel: 'stable', updateNotifications: true });
});

it('respects opt-out without network checks', async () => {
  updateGeneral({ updateNotifications: false });
  await checkUpdate();
  expect(checkForUpdate).not.toHaveBeenCalled();
});

it('selects the stored channel and exposes release notes through its resource', async () => {
  updateGeneral({ updateChannel: 'beta' });
  vi.mocked(checkForUpdate).mockResolvedValue({
    rid: 1,
    currentVersion: '0.1.0',
    version: '0.2.0',
    body: 'Release notes',
  });
  const hook = renderHook(useUpdate);
  await act(checkUpdate);
  expect(checkForUpdate).toHaveBeenCalledWith('beta');
  expect(hook.result.current.update?.version).toBe('0.2.0');
  act(dismissUpdate);
  expect(close).toHaveBeenCalledOnce();
});

it('closes late responses after consent is revoked', async () => {
  let finish:
    ((value: Awaited<ReturnType<typeof checkForUpdate>>) => void) | undefined;
  vi.mocked(checkForUpdate).mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const hook = renderHook(useUpdate);
  const checking = checkUpdate();
  act(() => {
    updateGeneral({ updateNotifications: false });
    dismissUpdate();
  });
  finish?.({ rid: 1, currentVersion: '0.1.0', version: '0.2.0', body: null });
  await act(async () => {
    await checking;
  });
  expect(hook.result.current.update).toBeNull();
  expect(close).toHaveBeenCalledOnce();
  expect(generalSettings.get().updateNotifications).toBe(false);
});

it('reports failures and no-update results', async () => {
  const hook = renderHook(useUpdate);
  vi.mocked(checkForUpdate).mockRejectedValue(
    new Error('Signature service unavailable'),
  );
  await act(checkUpdate);
  expect(hook.result.current.message).toBe('Signature service unavailable');
  vi.mocked(checkForUpdate).mockResolvedValue(null);
  await act(checkUpdate);
  expect(hook.result.current.message).toBe('OpenRows is up to date.');
});
