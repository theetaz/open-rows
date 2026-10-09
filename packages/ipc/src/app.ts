// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands, events } from './generated/bindings';
import type {
  MenuCommand,
  MenuItemState,
  UiReport,
} from './generated/bindings';
import { unwrap } from './connections';

/** Whether the previous run ended without a clean exit; `false` outside the shell. */
export async function crashRecoveryStatus(): Promise<boolean> {
  if (!isTauri()) return false;
  return commands.crashRecoveryStatus();
}

/** Show the log folder in the file manager (Help ▸ Open Logs Folder). */
export async function openLogsFolder(): Promise<void> {
  if (!isTauri()) return;
  await unwrap(commands.openLogsFolder());
}

/** Send one webview error or timing to the core log, where it is scrubbed. */
export async function reportUiEvent(report: UiReport): Promise<void> {
  if (!isTauri()) return;
  await commands.uiReport(report);
}

/**
 * The text of `settings.json` in the app-data dir; `null` before the first
 * save or outside the native shell.
 */
export async function readSettingsFile(): Promise<string | null> {
  if (!isTauri()) return null;
  return unwrap(commands.settingsRead());
}

/** Replace `settings.json` (a JSON object with `"openrows": 1`). */
export async function writeSettingsFile(text: string): Promise<void> {
  if (!isTauri()) return;
  await unwrap(commands.settingsWrite(text));
}

/**
 * Push enabled state and shortcuts to the native menu bar; resolves the ids
 * whose shortcut the platform rejected. A no-op outside the native shell.
 */
export async function syncMenu(
  states: readonly MenuItemState[],
): Promise<string[]> {
  if (!isTauri()) return [];
  return commands.menuSync([...states]);
}

/** Subscribe to native menu commands; resolves the unsubscribe function. */
export async function onMenuCommand(
  listener: (id: string) => void,
): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  return events.menuCommand.listen((event) => {
    const command: MenuCommand = event.payload;
    listener(command.id);
  });
}
