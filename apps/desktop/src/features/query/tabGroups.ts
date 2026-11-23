// SPDX-License-Identifier: Apache-2.0
import { updateTab } from './tabs';
import type { QueryTab, TabsState } from './tabs';

/** The eight colours of a tab group (`--or-group-<colour>`). */
export const GROUP_COLOURS = [
  'grey',
  'blue',
  'green',
  'yellow',
  'orange',
  'red',
  'pink',
  'purple',
] as const;
export type GroupColour = (typeof GROUP_COLOURS)[number];

/** A named, coloured run of adjacent tabs that can collapse to its chip. */
export type TabGroup = {
  id: string;
  name: string;
  colour: GroupColour;
  collapsed: boolean;
};

/** A copy of `tab` outside any group. */
export function withoutGroup(tab: QueryTab): QueryTab {
  const copy = { ...tab };
  delete copy.group;
  return copy;
}

/** Groups that still have tabs, in strip order. */
export function liveGroups(state: TabsState): TabGroup[] {
  const seen = new Set<string>();
  const live: TabGroup[] = [];
  for (const tab of state.tabs) {
    const group = tab.group ? state.groups?.[tab.group] : undefined;
    if (!group || seen.has(group.id)) continue;
    seen.add(group.id);
    live.push(group);
  }
  return live;
}

/** The first colour no live group uses (grey last), for a new group. */
function freeColour(state: TabsState): GroupColour {
  const used = new Set(liveGroups(state).map((group) => group.colour));
  const free = GROUP_COLOURS.filter((colour) => colour !== 'grey');
  return free.find((colour) => !used.has(colour)) ?? 'blue';
}

/** Move a tab to just after the last tab of `group` (or keep it when it has none). */
function afterGroup(
  tabs: readonly QueryTab[],
  tab: QueryTab,
  group: string,
): QueryTab[] {
  const rest = tabs.filter((item) => item.id !== tab.id);
  const last = rest.reduce(
    (found, item, index) => (item.group === group ? index : found),
    -1,
  );
  if (last < 0) return tabs.map((item) => (item.id === tab.id ? tab : item));
  return [...rest.slice(0, last + 1), tab, ...rest.slice(last + 1)];
}

/** Put `ids` in a new, unnamed group; returns the state and the group's id. */
export function newGroup(
  state: TabsState,
  ids: readonly string[],
): [TabsState, string] {
  const id = `group-${String(state.next)}`;
  const group: TabGroup = {
    id,
    name: '',
    colour: freeColour(state),
    collapsed: false,
  };
  const wanted = new Set(ids);
  const tabs = state.tabs.map((tab) =>
    wanted.has(tab.id) ? { ...tab, group: id, pinned: false } : tab,
  );
  // The members gather where the first of them is.
  const first = tabs.findIndex((tab) => tab.group === id);
  if (first < 0) return [state, id];
  const ordered = [
    ...tabs.slice(0, first),
    ...tabs.filter((tab) => tab.group === id),
    ...tabs.slice(first).filter((tab) => tab.group !== id),
  ];
  return [
    {
      ...state,
      tabs: ordered,
      next: state.next + 1,
      groups: { ...state.groups, [id]: group },
    },
    id,
  ];
}

/** Add a tab to an existing group, after its last tab; the group expands. */
export function addToGroup(
  state: TabsState,
  id: string,
  group: string,
): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  const target = state.groups?.[group];
  if (!tab || !target) return state;
  return {
    ...state,
    tabs: afterGroup(state.tabs, { ...tab, group, pinned: false }, group),
    groups: { ...state.groups, [group]: { ...target, collapsed: false } },
  };
}

/** Take a tab out of its group, placing it just after the group. */
export function removeFromGroup(state: TabsState, id: string): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab?.group) return state;
  return {
    ...state,
    tabs: afterGroup(state.tabs, withoutGroup(tab), tab.group),
  };
}

/** Dissolve a group; its tabs stay where they are. */
export function ungroup(state: TabsState, group: string): TabsState {
  const groups = Object.fromEntries(
    Object.entries(state.groups ?? {}).filter(([id]) => id !== group),
  );
  return {
    ...state,
    groups,
    tabs: state.tabs.map((tab) =>
      tab.group === group ? withoutGroup(tab) : tab,
    ),
  };
}

/** Rename, recolour or collapse a group. */
export function changeGroup(
  state: TabsState,
  group: string,
  change: Partial<Omit<TabGroup, 'id'>>,
): TabsState {
  const target = state.groups?.[group];
  if (!target) return state;
  return {
    ...state,
    groups: { ...state.groups, [group]: { ...target, ...change } },
  };
}

/** Ids of a group's tabs, in strip order. */
export function groupTabs(state: TabsState, group: string): string[] {
  return state.tabs.filter((tab) => tab.group === group).map((tab) => tab.id);
}

/**
 * After the tab at `index` moved: join a group when it sits between two of
 * its tabs, keep its group while touching it, otherwise leave it. Pinned
 * tabs never belong to a group.
 */
export function regroupAt(state: TabsState, index: number): TabsState {
  const tab = state.tabs[index];
  if (!tab) return state;
  const left = state.tabs[index - 1]?.group;
  const right = state.tabs[index + 1]?.group;
  let group: string | undefined;
  if (!tab.pinned && left && left === right) group = left;
  else if (
    !tab.pinned &&
    tab.group &&
    (left === tab.group || right === tab.group)
  )
    group = tab.group;
  if (group === tab.group) return state;
  return updateTab(state, tab.id, (item) =>
    group ? { ...item, group } : withoutGroup(item),
  );
}
