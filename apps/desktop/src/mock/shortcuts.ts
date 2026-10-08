// SPDX-License-Identifier: Apache-2.0
type Shortcut =
  | 'sidebar'
  | 'inspector'
  | 'new-tab'
  | 'close-tab'
  | 'database'
  | 'quick'
  | 'commands'
  /** Index of a workspace connection, from ⌘1 to ⌘9. */
  | number;
type KeyInput = Pick<
  KeyboardEvent,
  'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'code' | 'isComposing'
>;
const commands: Readonly<Record<string, Shortcut | undefined>> = {
  k: 'database',
  p: 'quick',
  t: 'new-tab',
  w: 'close-tab',
  ...Object.fromEntries(
    Array.from({ length: 9 }, (_, index) => [String(index + 1), index]),
  ),
};
/** Exact macOS modifiers; physical keys handle Option's altered character values. */
export function shortcut(event: KeyInput): Shortcut | undefined {
  if (isPalette(event)) return 'commands';
  if (!isCommand(event)) return undefined;
  if (event.ctrlKey && event.altKey) return undefined;
  if (event.ctrlKey) return event.code === 'KeyS' ? 'sidebar' : undefined;
  if (event.altKey) return event.code === 'KeyI' ? 'inspector' : undefined;
  return commands[event.key.toLowerCase()];
}
function isCommand(event: KeyInput): boolean {
  return event.metaKey && !event.shiftKey && !event.isComposing;
}

function isPalette(event: KeyInput): boolean {
  return (
    event.metaKey &&
    event.shiftKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !event.isComposing &&
    event.key.toLowerCase() === 'p'
  );
}
