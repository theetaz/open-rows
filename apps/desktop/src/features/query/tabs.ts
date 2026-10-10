// SPDX-License-Identifier: Apache-2.0
import type { ObjectKindDto } from '@openrows/ipc';
import type { SplitStates } from './splitState';
import type { ParamValues } from './paramValues';
import type { InspectionTarget } from '../inspection/target';

/**
 * Where a tab set lives: a saved connection and one of its databases. Tabs
 * run on that database's own session, so they keep their set across
 * reconnects and database switches.
 */
export type TabScope = { connectionId: string; database: string };

/** The relation a table tab browses; `database` is its real name. */
export type TableObject = {
  /** Catalog kind for favourite identity; older table tabs default to table. */
  kind?: ObjectKindDto;
  database: string;
  schema: string;
  name: string;
};

/**
 * The `.sql` file a query tab is bound to; `saved` is the text last written
 * or read, `error` why the last write failed.
 */
export type TabFile = {
  path: string;
  name: string;
  saved: string;
  error?: string;
  /** The file's modification time when it was last read or written. */
  modifiedMs?: number;
  /** Text another program saved to the file; Reload or Keep resolves it. */
  external?: string;
};

/**
 * A query, table or processes tab; a query tab's editor text lives here so it
 * survives tab switches. Table tabs (D-08) carry the browsed relation instead;
 * the scope's one processes tab (FR-ADMIN-01) lists the server's sessions.
 */
export type QueryTab = TabScope & {
  id: string;
  kind: 'query' | 'table' | 'processes' | 'inspection';
  title: string;
  sql: string;
  /** Schema object the tab was opened for, e.g. `['public', 'orders']`. */
  objectPath?: readonly string[];
  table?: TableObject;
  inspection?: InspectionTarget;
  file?: TabFile;
  /** Query parameter values of the parameters bar, by name (`:id`, `$1`, `?1`). */
  params?: ParamValues;
  /** Pinned tabs stay left and survive Close Others / Close Tabs to the Right. */
  pinned?: boolean;
  /**
   * The scope's one preview tab (a single-clicked table): the next preview
   * replaces it until it is edited or double-clicked.
   */
  preview?: boolean;
};

/**
 * What a new tab starts with; every field is optional. `table` makes a table
 * tab, `processes` the scope's processes tab.
 */
export type QueryTabInit = {
  sql?: string;
  title?: string;
  objectPath?: readonly string[];
  table?: TableObject;
  inspection?: InspectionTarget;
  file?: TabFile;
  /** Open as the scope's preview tab, replacing the current one. */
  preview?: boolean;
  processes?: true;
};

/** Every open query tab, the active tab per scope and the title counter. */
export type TabsState = {
  tabs: readonly QueryTab[];
  active: Readonly<Record<string, string | undefined>>;
  next: number;
  /** Descriptor-only split layouts; no editor instances or results. */
  splits?: SplitStates;
};

export const emptyTabs: TabsState = { tabs: [], active: {}, next: 1 };

/** Key of a scope in `TabsState.active`. */
export function scopeKey(scope: TabScope): string {
  return `${scope.connectionId}\n${scope.database}`;
}

export const inScope = (tab: QueryTab, scope: TabScope): boolean =>
  tab.connectionId === scope.connectionId && tab.database === scope.database;

/** Tabs of one scope, in opening order. */
export function tabsOf(state: TabsState, scope: TabScope): readonly QueryTab[] {
  return state.tabs.filter((tab) => inScope(tab, scope));
}

function kindOf(init: QueryTabInit): QueryTab['kind'] {
  if (init.inspection) return 'inspection';
  if (init.table) return 'table';
  return init.processes ? 'processes' : 'query';
}

/** Place `tab`: in the preview tab's slot when it replaces it, else last. */
function placed(
  tabs: readonly QueryTab[],
  tab: QueryTab,
  scope: TabScope,
): QueryTab[] {
  const index = tab.preview
    ? tabs.findIndex((item) => item.preview && inScope(item, scope))
    : -1;
  if (index < 0) return [...tabs, tab];
  return tabs.map((item, at) => (at === index ? tab : item));
}

/**
 * Add a tab to `scope` and make it that scope's active tab; a preview tab
 * replaces the scope's current preview tab in place.
 */
export function openTab(
  state: TabsState,
  scope: TabScope,
  init: QueryTabInit = {},
): TabsState {
  const kind = kindOf(init);
  if (kind === 'processes') {
    const existing = tabsOf(state, scope).find((tab) => tab.kind === kind);
    if (existing) return selectTab(state, existing.id);
  }
  const tab: QueryTab = {
    id: `${kind}-${String(state.next)}`,
    kind,
    title: init.title ?? `SQL Query ${String(state.next)}`,
    connectionId: scope.connectionId,
    database: scope.database,
    sql: init.sql ?? '',
    ...(init.objectPath ? { objectPath: init.objectPath } : {}),
    ...(init.table ? { table: init.table } : {}),
    ...(init.inspection ? { inspection: init.inspection } : {}),
    ...(init.file ? { file: init.file } : {}),
    ...(init.preview ? { preview: true } : {}),
  };
  return {
    ...state,
    tabs: placed(state.tabs, tab, scope),
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

/** Apply `change` to one tab. */
export function updateTab(
  state: TabsState,
  id: string,
  change: (tab: QueryTab) => QueryTab,
): TabsState {
  return {
    ...state,
    tabs: state.tabs.map((tab) => (tab.id === id ? change(tab) : tab)),
  };
}

/** Keep the latest editor text of a tab. */
export function editTab(state: TabsState, id: string, sql: string): TabsState {
  return updateTab(state, id, (tab) => ({ ...tab, sql, preview: false }));
}

/** Bind a tab to a `.sql` file (or record a save); the title follows the file. */
export function bindFile(
  state: TabsState,
  id: string,
  file: TabFile,
): TabsState {
  return updateTab(state, id, (tab) => ({ ...tab, file, title: file.name }));
}

/** Keep the parameter values of a tab's parameters bar. */
export function setTabParams(
  state: TabsState,
  id: string,
  params: ParamValues,
): TabsState {
  return updateTab(state, id, (tab) => ({ ...tab, params }));
}

/** Follow a committed `ALTER TABLE … RENAME TO`: the table tab's name and title. */
export function renameTableTab(
  state: TabsState,
  id: string,
  name: string,
): TabsState {
  return {
    ...state,
    tabs: state.tabs.map((tab) =>
      tab.id === id && tab.table
        ? {
            ...tab,
            title: name,
            objectPath: [tab.table.schema, name],
            table: { ...tab.table, name },
          }
        : tab,
    ),
  };
}
