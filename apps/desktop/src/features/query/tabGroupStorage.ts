// SPDX-License-Identifier: Apache-2.0
import { GROUP_COLOURS } from './tabGroups';
import type { GroupColour, TabGroup } from './tabGroups';
import type { TabsState } from './tabs';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const isColour = (value: unknown): value is GroupColour =>
  (GROUP_COLOURS as readonly unknown[]).includes(value);

/** The groups the stored tabs still use, for the next launch. */
export function storedGroups(
  state: TabsState,
  tabs: readonly { group?: string | undefined }[],
): TabGroup[] {
  const used = new Set(tabs.map((tab) => tab.group).filter(Boolean));
  return Object.values(state.groups ?? {}).filter((group) =>
    used.has(group.id),
  );
}

/** A stored group, or `null` when it is unreadable. */
function groupOf(value: unknown): TabGroup | null {
  if (!isRecord(value)) return null;
  const { id, name, colour, collapsed } = value;
  if (typeof id !== 'string' || typeof name !== 'string') return null;
  return {
    id,
    name: name.slice(0, 48),
    colour: isColour(colour) ? colour : 'blue',
    collapsed: collapsed === true,
  };
}

/** Stored groups by id; anything unreadable is dropped. */
export function groupsOf(value: unknown): Record<string, TabGroup> {
  if (!Array.isArray(value)) return {};
  const groups: Record<string, TabGroup> = {};
  for (const item of value) {
    const group = groupOf(item);
    if (group) groups[group.id] = group;
  }
  return groups;
}
