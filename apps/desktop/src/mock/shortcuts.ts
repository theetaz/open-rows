// SPDX-License-Identifier: Apache-2.0
type Shortcut =
  | 'sidebar'
  | 'inspector'
  | 'new-tab'
  | 'close-tab'
  | 'database'
  | 'quick'
  | 'commands'
  | 0
  | 1
  | 2
  | 3;
type KeyInput = Pick<
  KeyboardEvent,
  'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'code' | 'isComposing'
>;
const commands: Readonly<Record<string, Shortcut | undefined>> = {
  k: 'database',
  p: 'quick',
  t: 'new-tab',
  w: 'close-tab',
  '1': 0,
  '2': 1,
  '3': 2,
  '4': 3,
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
