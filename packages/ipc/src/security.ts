// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { commands } from './generated/bindings';
import { unwrap } from './connections';
import type { SecurityStatus } from './generated/bindings';

/** Lock state and OS authentication availability; demos cannot authenticate. */
export function securityStatus(): Promise<SecurityStatus> {
  return isTauri()
    ? commands.securityStatus()
    : Promise.resolve({
        locked: false,
        available: false,
        notice: 'OS authentication requires the desktop app.',
      });
}
/** Lock every application window. */
export function lockApp(): Promise<void> {
  return unwrap(commands.securityLock()).then(() => undefined);
}
/** Unlock through the OS-owned authentication prompt. */
export function unlockApp(): Promise<void> {
  return unwrap(commands.securityUnlock()).then(() => undefined);
}
/** Genuine input in a focused window resets inactivity. */
export function securityActivity(): Promise<void> {
  return isTauri() ? commands.securityActivity() : Promise.resolve();
}
/** Observe changes from any window and the backend inactivity timer. */
export function onSecurityChanged(
  listener: (locked: boolean) => void,
): Promise<() => void> {
  return isTauri()
    ? listen<boolean>('security-changed', (event) => {
        listener(event.payload);
      })
    : Promise.resolve(() => undefined);
}
