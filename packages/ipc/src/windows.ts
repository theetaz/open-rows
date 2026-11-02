// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { commands } from './generated/bindings';
import type { WindowEntryDto } from './generated/bindings';
import { unwrap } from './connections';

/** This webview's window label; `main` outside the native shell. */
export function currentWindowLabel(): string {
  return isTauri() ? getCurrentWindow().label : 'main';
}

/** Window ▸ New Window: an independent workspace window; resolves its label. */
export async function openNewWindow(): Promise<string | null> {
  return isTauri() ? unwrap(commands.windowNew()) : null;
}

/** Every open workspace window (for "Move to Window"); empty outside the shell. */
export async function listWindows(): Promise<WindowEntryDto[]> {
  return isTauri() ? commands.windowList() : [];
}

/**
 * Move serialised tabs to window `target`, or to a new window when `null`;
 * resolves the label of the window that received them.
 */
export async function transferTabs(
  target: string | null,
  payload: string,
): Promise<string> {
  if (!isTauri()) throw new Error('Tabs move between windows in the app');
  return unwrap(commands.windowTransfer(target, payload));
}

/** Tabs moved to this window since it last asked. */
export async function takeTransfers(): Promise<string[]> {
  return isTauri() ? commands.windowTransferTake() : [];
}

/** Call `listener` when tabs arrive; resolves the unsubscribe function. */
export async function onTransfersReady(
  listener: () => void,
): Promise<() => void> {
  return isTauri()
    ? listen('window-transfer-ready', listener)
    : () => undefined;
}

/** Name the native window (Window menu, Mission Control, Move to Window). */
export async function setWindowTitle(title: string): Promise<void> {
  if (isTauri()) await getCurrentWindow().setTitle(title);
}
