// SPDX-License-Identifier: Apache-2.0
import type { QueryTab, TabScope, TabsState } from '../query/tabs';

/** Launch-time `.sql` documents keep their own strip in the body. */
const LOCAL_FILES = 'local-files';

/** Every tab the window strip shows, in strip order. */
export function stripTabs(state: TabsState): QueryTab[] {
  return state.tabs.filter((tab) => tab.connectionId !== LOCAL_FILES);
}

/** Strip tabs not hidden in a collapsed group: what ⌘1–9 and ⌃Tab walk. */
export function visibleTabs(state: TabsState): QueryTab[] {
  return stripTabs(state).filter((tab) => {
    const group = tab.group ? state.groups?.[tab.group] : undefined;
    return !group?.collapsed;
  });
}

/**
 * The tab the window shows: a window tab (the connection flow) when one is
 * current, else the active tab of the browsed scope.
 */
export function shownTab(
  state: TabsState,
  scope: TabScope | undefined,
): QueryTab | undefined {
  const current = state.current
    ? state.tabs.find((tab) => tab.id === state.current)
    : undefined;
  if (current) return current;
  if (!scope) return undefined;
  const id = state.active[`${scope.connectionId}\n${scope.database}`];
  return state.tabs.find((tab) => tab.id === id);
}

/** The visible tab `n` places from `id`, wrapping; ⌘9 asks for the last. */
export function stepFrom(
  state: TabsState,
  id: string | undefined,
  step: number,
): QueryTab | undefined {
  const list = visibleTabs(state);
  if (!list.length) return undefined;
  const at = list.findIndex((tab) => tab.id === id);
  const from = at < 0 ? 0 : at;
  return list[(((from + step) % list.length) + list.length) % list.length];
}

/** ⌘1–⌘8 by position, ⌘9 the last tab. */
export function tabAtKey(state: TabsState, key: number): QueryTab | undefined {
  const list = visibleTabs(state);
  return key >= 9 ? list.at(-1) : list[key - 1];
}

/** Whether a tab belongs to another connection or database than `scope`. */
export function outsideScope(
  tab: QueryTab,
  scope: TabScope | undefined,
): boolean {
  if (tab.kind === 'connection') return false;
  return (
    !scope ||
    tab.connectionId !== scope.connectionId ||
    tab.database !== scope.database
  );
}
