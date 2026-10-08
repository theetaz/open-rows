// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';
import { commands } from './generated/bindings';
import type { FileEncodingDto } from './generated/bindings';
import { unwrap } from './connections';

/** Text of a destructive confirmation; `action` labels the confirming button. */
export type DestructivePrompt = {
  title: string;
  message: string;
  action: string;
};

/**
 * Ask with a native macOS alert (NSAlert via the scoped dialog plugin,
 * docs/12 §5). Resolves `null` outside the native shell so callers can fall
 * back to an in-app alert sheet.
 */
export async function confirmDestructive(
  prompt: DestructivePrompt,
): Promise<boolean | null> {
  if (!isTauri()) return null;
  return ask(prompt.message, {
    title: prompt.title,
    kind: 'warning',
    okLabel: prompt.action,
    cancelLabel: 'Cancel',
  });
}

/**
 * "Add a file…": Rust shows the native open panel and reads the picked file
 * (16 MiB cap; text must be UTF-8), so the webview needs no file-system
 * permission. Resolves the field value text, or `null` when cancelled or
 * outside the native shell.
 */
export async function pickFileValue(
  encoding: FileEncodingDto,
): Promise<string | null> {
  if (!isTauri()) return null;
  return unwrap(commands.tableEditFile(encoding));
}
