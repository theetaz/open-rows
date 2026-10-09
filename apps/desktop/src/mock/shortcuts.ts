// SPDX-License-Identifier: Apache-2.0
import { commandForEvent } from '../features/commands';

/** A command id, or the index of a workspace connection (⌘1 to ⌘9). */
type Shortcut = string | number;
type KeyInput = Pick<
  KeyboardEvent,
  'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'code' | 'isComposing'
>;

/** Commands the app shell handles itself, so Settings opens over the gallery too. */
const SHELL_ONLY = new Set(['settings']);

/** ⌘1–⌘9: the connection at that rail position. */
function connectionIndex(event: KeyInput): number | undefined {
  if (!event.metaKey || event.shiftKey || event.altKey || event.ctrlKey)
    return undefined;
  return /^[1-9]$/.test(event.key) ? Number(event.key) - 1 : undefined;
}

/** The bound command (or connection) of a key press, with exact modifiers. */
export function shortcut(event: KeyInput): Shortcut | undefined {
  if (event.isComposing) return undefined;
  const index = connectionIndex(event);
  if (index !== undefined) return index;
  const id = commandForEvent(event);
  return id && !SHELL_ONLY.has(id) ? id : undefined;
}
