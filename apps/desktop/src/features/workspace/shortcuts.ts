// SPDX-License-Identifier: Apache-2.0
import { commandForEvent } from '../commands';

/** A command id, or the index of a workspace connection (⌥⌘1 to ⌥⌘9). */
type Shortcut = string | number;
type KeyInput = Pick<
  KeyboardEvent,
  'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'code' | 'isComposing'
>;

/** Commands the app shell handles itself, so Settings opens over the gallery too. */
const SHELL_ONLY = new Set(['settings']);

/** The digit of a number-row key, from its physical key (⌥ changes `key`). */
function digitOf(event: KeyInput): number | undefined {
  const digit =
    /^Digit([1-9])$/.exec(event.code)?.[1] ?? /^[1-9]$/.exec(event.key)?.[0];
  return digit ? Number(digit) : undefined;
}

/** ⌘1–⌘8: the tab at that position, ⌘9 the last tab (Chrome). */
function tabKey(event: KeyInput): string | undefined {
  if (!event.metaKey || event.shiftKey || event.altKey || event.ctrlKey)
    return undefined;
  const digit = digitOf(event);
  return digit ? `select-tab-${String(digit)}` : undefined;
}

/** ⌥⌘1–⌥⌘9: the connection at that rail position. */
function connectionIndex(event: KeyInput): number | undefined {
  if (!event.metaKey || !event.altKey || event.shiftKey || event.ctrlKey)
    return undefined;
  const digit = digitOf(event);
  return digit ? digit - 1 : undefined;
}

/** ⌃Tab and ⌃⇧Tab walk the tabs, as in every macOS browser. */
function tabCycle(event: KeyInput): string | undefined {
  if (event.key !== 'Tab' || !event.ctrlKey || event.metaKey || event.altKey)
    return undefined;
  return event.shiftKey ? 'previous-tab' : 'next-tab';
}

/** The bound command (or connection) of a key press, with exact modifiers. */
export function shortcut(event: KeyInput): Shortcut | undefined {
  if (event.isComposing) return undefined;
  const fixed = tabKey(event) ?? tabCycle(event);
  if (fixed) return fixed;
  const index = connectionIndex(event);
  if (index !== undefined) return index;
  const id = commandForEvent(event);
  return id && !SHELL_ONLY.has(id) ? id : undefined;
}
