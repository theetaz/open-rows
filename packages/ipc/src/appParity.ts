// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { commands } from './generated/bindings';
import type { LocationsDto, NativeFileDto } from './generated/bindings';
import { unwrap } from './connections';

/** Current native data/history/log paths; browser preview has no local storage. */
export async function getDataLocations(): Promise<LocationsDto | null> {
  return isTauri() ? unwrap(commands.dataLocations()) : null;
}
/** Choose a local data folder using the platform dialog. */
export async function chooseDataLocation(): Promise<string | null> {
  return isTauri() ? unwrap(commands.dataLocationChoose()) : null;
}
/** Validate and schedule a user-approved offline data copy for restart. */
export async function scheduleDataLocation(path: string): Promise<void> {
  if (!isTauri())
    throw new Error('Data directories can only change in the native app');
  await unwrap(commands.dataLocationApply(path));
}
/** Drain OS file-open requests, receiving content only after native validation. */
export async function takeNativeFiles(): Promise<NativeFileDto[]> {
  return isTauri() ? unwrap(commands.nativeFilesTake()) : [];
}
/** Watch document handoffs; listeners still perform an initial drain on launch. */
export async function onNativeFilesReady(
  listener: () => void,
): Promise<() => void> {
  return isTauri() ? listen('native-files-ready', listener) : () => undefined;
}
/** Reveal a saved connection's database file using the platform file manager. */
export async function revealDatabase(profileId: string): Promise<void> {
  if (isTauri()) await unwrap(commands.databaseReveal(profileId));
}
