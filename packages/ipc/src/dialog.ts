// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { ask } from '@tauri-apps/plugin-dialog';

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
