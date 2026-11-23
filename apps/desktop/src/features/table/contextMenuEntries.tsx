// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import type { IconName, MenuEntry } from '@openrows/ui';
import { displayCombo } from '../commands';
import type { Bindings } from '../commands/bindings';
import type { Action } from './contextMenuActions';

type Submenu = Extract<MenuEntry, { kind: 'submenu' }>;
type Built = { entries: MenuEntry[]; groups: Map<string, MenuEntry[]> };

/** The shortcut a row shows: a fixed hint, or its command's binding. */
function shortcutOf(action: Action, bindings: Bindings): string | undefined {
  if (action.shortcut) return action.shortcut;
  const combo = action.command ? bindings[action.command] : null;
  return combo ? displayCombo(combo) : undefined;
}

function itemOf(action: Action, bindings: Bindings): MenuEntry {
  const shortcut = shortcutOf(action, bindings);
  return {
    kind: 'item',
    label: action.label,
    run: action.run,
    disabled: action.disabled ?? false,
    danger: action.danger ?? false,
    ...(action.icon ? { icon: <Icon name={action.icon} /> } : {}),
    ...(shortcut ? { shortcut } : {}),
  };
}

/** A separator before a new group, never first and never doubled. */
export function separate(entries: MenuEntry[]): void {
  const last = entries[entries.length - 1];
  if (last && last.kind !== 'separator') entries.push({ kind: 'separator' });
}

function add(built: Built, action: Action, bindings: Bindings): void {
  const { group } = action;
  const rows = group ? built.groups.get(group.label) : undefined;
  if (rows) {
    rows.push(itemOf(action, bindings));
    return;
  }
  if (action.separated) separate(built.entries);
  if (!group) {
    built.entries.push(itemOf(action, bindings));
    return;
  }
  const entries = [itemOf(action, bindings)];
  built.groups.set(group.label, entries);
  built.entries.push({
    kind: 'submenu',
    label: group.label,
    ...(group.icon ? { icon: <Icon name={group.icon} /> } : {}),
    entries,
  });
}

/**
 * Menu rows for the actions in order, with glyphs and shortcuts: separators
 * between groups, and actions naming a `group` collected into one submenu
 * at the first of them. `groups` holds each submenu's rows for additions.
 */
export function actionEntries(
  actions: readonly Action[],
  bindings: Bindings,
): Built {
  const built: Built = { entries: [], groups: new Map() };
  for (const action of actions) add(built, action, bindings);
  return built;
}

/** A submenu row with a glyph, for entries built elsewhere. */
export function withGlyph(entry: MenuEntry, icon: IconName): MenuEntry {
  return entry.kind === 'submenu'
    ? ({ ...entry, icon: <Icon name={icon} /> } satisfies Submenu)
    : entry;
}
