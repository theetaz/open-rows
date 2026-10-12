// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import type { Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { generalSettings } from './general';

type Context = { update: Update | null; canInstall: boolean };
type Installation = {
  installing: boolean;
  error: string;
  start: () => Promise<void>;
};

/** Keep one verified payload; recheck consent and workspace safety before install. */
export function useUpdateInstall(
  update: Update | null,
  canInstall: boolean,
): Installation {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState('');
  const latest = useRef({ update, canInstall });
  const downloaded = useRef<Update | null>(null);
  useEffect(() => {
    latest.current = { update, canInstall };
  }, [update, canInstall]);
  useEffect(
    () => () => {
      latest.current = { update: null, canInstall: false };
    },
    [],
  );
  const start = async (): Promise<void> => {
    if (!update || !canInstall || installing) return;
    setInstalling(true);
    setError('');
    try {
      await install(update, latest, downloaded);
    } catch (failure) {
      setError(
        failure instanceof Error
          ? failure.message
          : 'Update installation failed.',
      );
      setInstalling(false);
    }
  };
  return { installing, error, start };
}

async function install(
  update: Update,
  latest: RefObject<Context>,
  downloaded: RefObject<Update | null>,
): Promise<void> {
  if (downloaded.current !== update) {
    await update.download(undefined, { timeout: 30000 });
    downloaded.current = update;
  }
  if (
    !generalSettings.get().updateNotifications ||
    latest.current.update !== update
  ) {
    await update.close().catch(() => undefined);
    throw new Error('Update settings changed. Check for updates again.');
  }
  if (!latest.current.canInstall)
    throw new Error(
      'Workspace changed. Close all tabs and disconnect before installing.',
    );
  await update.install();
  await relaunch();
}
