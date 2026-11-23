// SPDX-License-Identifier: Apache-2.0
import {
  back,
  highlight,
  levelEntries,
  move,
  openSubmenu,
  step,
  typeahead,
} from './menuModel';
import type { MenuEntry, MenuState } from './menuModel';

/** What a key does to a menu: a new state, choose the highlighted row, or close. */
type KeyResult =
  | { kind: 'state'; state: MenuState }
  | { kind: 'choose'; level: number; index: number }
  | { kind: 'close' }
  | { kind: 'none' };

function deepest(state: MenuState): { level: number; index: number | null } {
  const level = state.active.length - 1;
  return { level, index: state.active[level] ?? null };
}

function intoSubmenu(
  state: MenuState,
  entries: readonly MenuEntry[],
): KeyResult {
  const { level, index } = deepest(state);
  const entry = index === null ? undefined : entries[index];
  if (index === null || entry?.kind !== 'submenu' || entry.disabled)
    return { kind: 'none' };
  const first = step(entry.entries, null, 1);
  return { kind: 'state', state: openSubmenu(state, level, index, first) };
}

/**
 * ↑↓ (wrapping), Home/End, → into a submenu on its first row, ← back,
 * ↩ / Space choose, Esc or Tab close, and type-ahead by first letters.
 */
export function menuKey(
  key: string,
  state: MenuState,
  root: readonly MenuEntry[],
  typed: string,
): KeyResult {
  const entries = levelEntries(root, state.open)[state.active.length - 1] ?? [];
  const set = (next: MenuState): KeyResult => ({ kind: 'state', state: next });
  const { level, index } = deepest(state);
  switch (key) {
    case 'ArrowDown':
      return set(move(state, entries, 1));
    case 'ArrowUp':
      return set(move(state, entries, -1));
    case 'Home':
      return set(highlight(state, step(entries, null, 1)));
    case 'End':
      return set(highlight(state, step(entries, null, -1)));
    case 'ArrowRight':
      return intoSubmenu(state, entries);
    case 'ArrowLeft':
      return state.open.length ? set(back(state)) : { kind: 'none' };
    case 'Enter':
    case ' ':
      return index === null
        ? { kind: 'none' }
        : { kind: 'choose', level, index };
    case 'Escape':
    case 'Tab':
      return { kind: 'close' };
    default:
      return typed
        ? set(highlight(state, typeahead(entries, typed)))
        : { kind: 'none' };
  }
}
