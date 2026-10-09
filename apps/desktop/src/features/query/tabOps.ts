// SPDX-License-Identifier: Apache-2.0
import { inScope, tabsOf, updateTab } from './tabs';
import type { QueryTab, TabsState } from './tabs';

/** Pinned tabs first, each group in its current order. */
const pinnedFirst = (tabs: readonly QueryTab[]): QueryTab[] => [
  ...tabs.filter((tab) => tab.pinned),
  ...tabs.filter((tab) => !tab.pinned),
];

/** Replace the order of one scope's tabs, leaving other scopes in place. */
function withScopeOrder(
  state: TabsState,
  sample: QueryTab,
  ordered: readonly QueryTab[],
): TabsState {
  let next = 0;
  const tabs = state.tabs.map((tab) =>
    inScope(tab, sample) ? (ordered[next++] ?? tab) : tab,
  );
  return { ...state, tabs };
}

/** Move a tab to `index` among its scope's tabs; pinned tabs stay first. */
export function moveTab(
  state: TabsState,
  id: string,
  index: number,
): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  const rest = tabsOf(state, tab).filter((item) => item.id !== id);
  const at = Math.max(0, Math.min(index, rest.length));
  const ordered = [...rest.slice(0, at), tab, ...rest.slice(at)];
  return withScopeOrder(state, tab, pinnedFirst(ordered));
}

/** Pin (moving it after the other pinned tabs) or unpin a tab. */
export function pinTab(
  state: TabsState,
  id: string,
  pinned: boolean,
): TabsState {
  const changed = updateTab(state, id, (tab) => ({
    ...tab,
    pinned,
    preview: false,
  }));
  const tab = changed.tabs.find((item) => item.id === id);
  if (!tab) return state;
  const rest = tabsOf(changed, tab).filter((item) => item.id !== id);
  // Either way the tab lands on the boundary between pinned and unpinned tabs.
  const count = rest.filter((item) => item.pinned).length;
  const slot = [...rest.slice(0, count), tab, ...rest.slice(count)];
  return withScopeOrder(changed, tab, slot);
}

/** A preview tab becomes a normal tab (edited or double-clicked). */
export function keepTab(state: TabsState, id: string): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab?.preview) return state;
  return updateTab(state, id, (item) => ({ ...item, preview: false }));
}

/** Unpinned tabs Close Others closes: every other tab of the scope. */
export function otherTabs(state: TabsState, id: string): string[] {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return [];
  return tabsOf(state, tab)
    .filter((item) => item.id !== id && !item.pinned)
    .map((item) => item.id);
}

/** Unpinned tabs Close Tabs to the Right closes. */
export function tabsRightOf(state: TabsState, id: string): string[] {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return [];
  const siblings = tabsOf(state, tab);
  return siblings
    .slice(siblings.findIndex((item) => item.id === id) + 1)
    .filter((item) => !item.pinned)
    .map((item) => item.id);
}
