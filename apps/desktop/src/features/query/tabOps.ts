// SPDX-License-Identifier: Apache-2.0
import { openTab, updateTab } from './tabs';
import type { QueryTab, TabsState } from './tabs';
import { regroupAt, withoutGroup } from './tabGroups';

/** Pinned tabs first, each set in its current order. */
const pinnedFirst = (tabs: readonly QueryTab[]): QueryTab[] => [
  ...tabs.filter((tab) => tab.pinned),
  ...tabs.filter((tab) => !tab.pinned),
];

/**
 * Move a tab to `index` among the window's tabs; pinned tabs stay first.
 * Dropped between two tabs of a group it joins it; past its ends it leaves.
 */
export function moveTab(
  state: TabsState,
  id: string,
  index: number,
): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  const rest = state.tabs.filter((item) => item.id !== id);
  const at = Math.max(0, Math.min(index, rest.length));
  const tabs = pinnedFirst([...rest.slice(0, at), tab, ...rest.slice(at)]);
  return regroupAt(
    { ...state, tabs },
    tabs.findIndex((item) => item.id === id),
  );
}

/** Move a tab to the start (after the pinned tabs) or the end of the strip. */
export function moveToEdge(
  state: TabsState,
  id: string,
  edge: 'start' | 'end',
): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  const loose = withoutGroup(tab);
  const rest = state.tabs.filter((item) => item.id !== id);
  const pinned = rest.filter((item) => item.pinned).length;
  const at = edge === 'end' ? rest.length : tab.pinned ? 0 : pinned;
  return { ...state, tabs: [...rest.slice(0, at), loose, ...rest.slice(at)] };
}

/** Pin (moving it after the other pinned tabs, out of its group) or unpin a tab. */
export function pinTab(
  state: TabsState,
  id: string,
  pinned: boolean,
): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  const changed = { ...withoutGroup(tab), pinned, preview: false };
  const rest = state.tabs.filter((item) => item.id !== id);
  // Either way the tab lands on the boundary between pinned and unpinned tabs.
  const count = rest.filter((item) => item.pinned).length;
  return {
    ...state,
    tabs: [...rest.slice(0, count), changed, ...rest.slice(count)],
  };
}

/** A copy of a tab right after it, in its group; file bindings stay with the original. */
export function duplicateTab(state: TabsState, id: string): TabsState {
  const index = state.tabs.findIndex((item) => item.id === id);
  const tab = state.tabs[index];
  if (!tab || tab.kind === 'connection') return state;
  const copy: QueryTab = {
    ...tab,
    id: `${tab.kind}-${String(state.next)}`,
    preview: false,
    pinned: false,
  };
  delete copy.file;
  return {
    ...state,
    tabs: [
      ...state.tabs.slice(0, index + 1),
      copy,
      ...state.tabs.slice(index + 1),
    ],
    next: state.next + 1,
    active: { ...state.active },
  };
}

/** A preview tab becomes a normal tab (edited or double-clicked). */
export function keepTab(state: TabsState, id: string): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab?.preview) return state;
  return updateTab(state, id, (item) => ({ ...item, preview: false }));
}

/**
 * Rename a tab (it stops being a preview tab); a blank title changes
 * nothing. Saving a file-bound tab names it after its file again.
 */
export function renameTab(
  state: TabsState,
  id: string,
  title: string,
): TabsState {
  const trimmed = title.trim();
  if (!trimmed) return state;
  return updateTab(state, id, (tab) => ({
    ...tab,
    title: trimmed,
    preview: false,
  }));
}

/** Unpinned tabs Close Others closes: every other tab of the window. */
export function otherTabs(state: TabsState, id: string): string[] {
  if (!state.tabs.some((item) => item.id === id)) return [];
  return state.tabs
    .filter((item) => item.id !== id && !item.pinned)
    .map((item) => item.id);
}

/** Unpinned tabs Close Tabs to the Right closes. */
export function tabsRightOf(state: TabsState, id: string): string[] {
  const index = state.tabs.findIndex((item) => item.id === id);
  if (index < 0) return [];
  return state.tabs
    .slice(index + 1)
    .filter((item) => !item.pinned)
    .map((item) => item.id);
}

/**
 * A new query tab of `id`'s connection and database right after it, in its
 * group; it becomes the active tab of its scope.
 */
export function openBeside(state: TabsState, id: string): TabsState {
  const index = state.tabs.findIndex((item) => item.id === id);
  const tab = state.tabs[index];
  if (!tab) return state;
  const opened = openTab(state, tab);
  const added = opened.tabs.at(-1);
  if (!added) return opened;
  const moved = moveTab(opened, added.id, index + 1);
  const { group } = tab;
  return group
    ? updateTab(moved, added.id, (item) => ({ ...item, group }))
    : moved;
}
