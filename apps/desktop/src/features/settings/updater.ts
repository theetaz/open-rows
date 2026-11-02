// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { Update } from '@tauri-apps/plugin-updater';
import { checkForUpdate } from '@openrows/ipc';
import { generalSettings } from './general';
import { updateFeed } from './updateFeed';

type Snapshot = { update: Update | null; checking: boolean; message: string };
let snapshot: Snapshot = { update: null, checking: false, message: '' };
const listeners = new Set<() => void>();
let generation = 0;

function publish(change: Partial<Snapshot>): void {
  snapshot = { ...snapshot, ...change };
  listeners.forEach((listener) => {
    listener();
  });
}

/** Shared manual-check status and the non-blocking update notice. */
export function useUpdate(): Snapshot {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => snapshot,
  );
}

/** Release any updater resource when dismissed, disabled, or changing channels. */
export function dismissUpdate(): void {
  generation += 1;
  const update = snapshot.update;
  publish({ update: null, checking: false, message: '' });
  void update?.close().catch(() => undefined);
}

/** Check only enabled, signed feeds; late replies cannot bypass an opt-out. */
export async function checkUpdate(): Promise<void> {
  const general = generalSettings.get();
  if (!general.updateNotifications || !updateFeed(general.updateChannel))
    return;
  dismissUpdate();
  const token = generation;
  publish({ checking: true, message: 'Checking for updates…' });
  try {
    const result = await checkForUpdate(general.updateChannel);
    const update = result
      ? new Update({ ...result, body: result.body ?? '', rawJson: {} })
      : null;
    const current = generalSettings.get();
    if (
      token !== generation ||
      !current.updateNotifications ||
      current.updateChannel !== general.updateChannel
    ) {
      await update?.close();
      if (token === generation) publish({ checking: false, message: '' });
      return;
    }
    publish({
      update,
      checking: false,
      message: update
        ? `OpenRows ${update.version} is available.`
        : 'OpenRows is up to date.',
    });
  } catch (error) {
    if (token === generation)
      publish({
        checking: false,
        message:
          error instanceof Error ? error.message : 'Update check failed.',
      });
  }
}
