// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Run `allow` when the user closes the window (close button, ⌘W); returning
 * `false` keeps the window open so the app can ask about unsaved work first.
 * Resolves the unlisten function; a no-op outside the native shell.
 */
export async function onWindowClose(allow: () => boolean): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  return getCurrentWindow().onCloseRequested((event) => {
    if (!allow()) event.preventDefault();
  });
}

/** Close the window without asking again, once the user confirmed. */
export async function closeWindow(): Promise<void> {
  if (!isTauri()) return;
  await getCurrentWindow().destroy();
}
