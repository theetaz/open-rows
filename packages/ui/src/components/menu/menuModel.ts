// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';

/** One menu row: a command, a submenu (▸), a separator or a section heading. */
export type MenuEntry =
  | {
      kind: 'item';
      label: string;
      run: () => void;
      /** A 14 px icon in the leading column. */
      icon?: ReactNode;
      /** Shortcut shown right-aligned, e.g. `⌘I`. */
      shortcut?: string;
      /** A check mark; with `radio` the row is one choice of a group. */
      checked?: boolean;
      radio?: boolean;
      /** A colour swatch in the leading column (a CSS colour, e.g. `var(--or-group-blue)`). */
      swatch?: string;
      disabled?: boolean;
      /** Destructive command: red, and highlighted in the destructive fill. */
      danger?: boolean;
    }
  | {
      kind: 'submenu';
      label: string;
      entries: readonly MenuEntry[];
      icon?: ReactNode;
      disabled?: boolean;
    }
  | { kind: 'separator' }
  | { kind: 'header'; label: string };

/**
 * Open submenus (`open[k]` is the row of level `k` whose submenu is level
 * `k + 1`) and the highlighted row of every level.
 */
export type MenuState = {
  open: readonly number[];
  active: readonly (number | null)[];
};

/** Rows the pointer and the keys can land on. */
function selectable(entry: MenuEntry | undefined): boolean {
  if (!entry) return false;
  if (entry.kind !== 'item' && entry.kind !== 'submenu') return false;
  return !entry.disabled;
}

/** The entries of each open level, the root first. */
export function levelEntries(
  root: readonly MenuEntry[],
  open: readonly number[],
): (readonly MenuEntry[])[] {
  const levels = [root];
  for (const [depth, index] of open.entries()) {
    const entry = levels[depth]?.[index];
    if (entry?.kind !== 'submenu') break;
    levels.push(entry.entries);
  }
  return levels;
}

/** The next selectable row from `from` in direction `dir`, wrapping. */
export function step(
  entries: readonly MenuEntry[],
  from: number | null,
  dir: 1 | -1,
): number | null {
  const count = entries.length;
  const start = from ?? (dir === 1 ? -1 : count);
  for (let offset = 1; offset <= count; offset += 1) {
    const index = (((start + dir * offset) % count) + count) % count;
    if (selectable(entries[index])) return index;
  }
  return null;
}

/** First selectable row whose label starts with `prefix` (type-ahead). */
export function typeahead(
  entries: readonly MenuEntry[],
  prefix: string,
): number | null {
  const wanted = prefix.toLowerCase();
  const index = entries.findIndex(
    (entry) =>
      selectable(entry) &&
      'label' in entry &&
      entry.label.toLowerCase().startsWith(wanted),
  );
  return index < 0 ? null : index;
}

function replaceLast<T>(values: readonly T[], value: T): T[] {
  return [...values.slice(0, -1), value];
}

/** The pointer is over `index` of `level` (or over no row). */
export function hover(
  state: MenuState,
  level: number,
  index: number | null,
): MenuState {
  if (index !== null && state.open[level] === index)
    return {
      open: state.open,
      active: state.active.map((value, depth) =>
        depth === level ? index : value,
      ),
    };
  return {
    open: state.open.slice(0, level),
    active: [...state.active.slice(0, level), index],
  };
}

/** Open the submenu of `index` at `level`; `first` is its highlighted row. */
export function openSubmenu(
  state: MenuState,
  level: number,
  index: number,
  first: number | null,
): MenuState {
  return {
    open: [...state.open.slice(0, level), index],
    active: [...state.active.slice(0, level), index, first],
  };
}

/** Close the deepest submenu; its parent row stays highlighted. */
export function back(state: MenuState): MenuState {
  if (!state.open.length) return state;
  return { open: state.open.slice(0, -1), active: state.active.slice(0, -1) };
}

/** Move the deepest level's highlight. */
export function move(
  state: MenuState,
  entries: readonly MenuEntry[],
  dir: 1 | -1,
): MenuState {
  const current = state.active[state.active.length - 1] ?? null;
  return {
    ...state,
    active: replaceLast(state.active, step(entries, current, dir)),
  };
}

/** Highlight `index` on the deepest level (Home, End, type-ahead). */
export function highlight(state: MenuState, index: number | null): MenuState {
  if (index === null) return state;
  return { ...state, active: replaceLast(state.active, index) };
}
