// SPDX-License-Identifier: Apache-2.0
/**
 * Where a tab set lives: a saved connection and one of its databases. Tabs
 * run on that database's own session, so they keep their set across
 * reconnects and database switches.
 */
export type TabScope = { connectionId: string; database: string };

/** A query tab; its editor text lives here so it survives tab switches. */
export type QueryTab = TabScope & {
  id: string;
  title: string;
  sql: string;
  /** Schema object the tab was opened for, e.g. `['public', 'orders']`. */
  objectPath?: readonly string[];
};

/** What a new tab starts with; every field is optional. */
export type QueryTabInit = {
  sql?: string;
  title?: string;
  objectPath?: readonly string[];
};

/** Every open query tab, the active tab per scope and the title counter. */
export type TabsState = {
  tabs: readonly QueryTab[];
  active: Readonly<Record<string, string | undefined>>;
  next: number;
};

export const emptyTabs: TabsState = { tabs: [], active: {}, next: 1 };

/** Key of a scope in `TabsState.active`. */
export function scopeKey(scope: TabScope): string {
  return `${scope.connectionId}\n${scope.database}`;
}

const inScope = (tab: QueryTab, scope: TabScope): boolean =>
  tab.connectionId === scope.connectionId && tab.database === scope.database;

/** Tabs of one scope, in opening order. */
export function tabsOf(state: TabsState, scope: TabScope): readonly QueryTab[] {
  return state.tabs.filter((tab) => inScope(tab, scope));
}

/** Append a tab to `scope` and make it that scope's active tab. */
export function openTab(
  state: TabsState,
  scope: TabScope,
  init: QueryTabInit = {},
): TabsState {
  const tab: QueryTab = {
    id: `query-${String(state.next)}`,
    title: init.title ?? `SQL Query ${String(state.next)}`,
    connectionId: scope.connectionId,
    database: scope.database,
    sql: init.sql ?? '',
    ...(init.objectPath ? { objectPath: init.objectPath } : {}),
  };
  return {
    tabs: [...state.tabs, tab],
    active: { ...state.active, [scopeKey(scope)]: tab.id },
    next: state.next + 1,
  };
}

/** Close a tab; its scope focuses the nearest remaining neighbour. */
export function closeTab(state: TabsState, id: string): TabsState {
  const closing = state.tabs.find((tab) => tab.id === id);
  if (!closing) return state;
  const key = scopeKey(closing);
  const siblings = tabsOf(state, closing);
  const index = siblings.findIndex((tab) => tab.id === id);
  const remaining = siblings.filter((tab) => tab.id !== id);
  const active =
    state.active[key] === id
      ? remaining[Math.max(0, index - 1)]?.id
      : state.active[key];
  return {
    ...state,
    tabs: state.tabs.filter((tab) => tab.id !== id),
    active: { ...state.active, [key]: active },
  };
}

/** Focus a tab within its scope. */
export function selectTab(state: TabsState, id: string): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  return { ...state, active: { ...state.active, [scopeKey(tab)]: id } };
}

/** Keep the latest editor text of a tab. */
export function editTab(state: TabsState, id: string, sql: string): TabsState {
  return {
    ...state,
    tabs: state.tabs.map((tab) => (tab.id === id ? { ...tab, sql } : tab)),
  };
}
