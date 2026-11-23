// SPDX-License-Identifier: Apache-2.0
import type { ObjectKindDto } from '@openrows/ipc';
import type { SplitStates } from './splitState';
import type { ParamValues } from './paramValues';
import type { InspectionTarget } from '../inspection/target';
import type { TabGroup } from './tabGroups';
import { t } from './initialMessages';

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

/** A place a table tab has shown, for its back and forward history. */
export type TabPlace = TabScope & { table: TableObject };

/**
 * The connection flow a `connection` tab shows: new (optionally for a database
 * file opened from Finder) or editing a profile (optionally at its safety policy).
 */
export type ConnectionFlowTarget = {
  mode: 'new' | 'edit';
  profileId?: string;
  engine?: 'sqlite' | 'duckdb';
  filePath?: string;
  focus?: 'safety';
};

/** Window tabs that belong to no connection: the connection flow. */
export const WINDOW_SCOPE: TabScope = { connectionId: '', database: '' };

/**
 * A query, table or processes tab; a query tab's editor text lives here so it
 * survives tab switches. Table tabs (D-08) carry the browsed relation instead;
 * the scope's one processes tab (FR-ADMIN-01) lists the server's sessions; a
 * connection tab shows the new or edit connection flow.
 */
export type QueryTab = TabScope & {
  id: string;
  kind:
    'query' | 'table' | 'processes' | 'monitor' | 'inspection' | 'connection';
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
  /** The id of the tab group the tab belongs to. */
  group?: string;
  /** Places a table tab showed before (back) and after (forward) this one. */
  back?: readonly TabPlace[];
  forward?: readonly TabPlace[];
  /** What a connection tab shows. */
  flow?: ConnectionFlowTarget;
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
  /** Open the scope’s one Monitor workspace. */
  monitor?: true;
  /** Open (or refocus) the connection flow as a window tab. */
  flow?: ConnectionFlowTarget;
};

/** Every open query tab, the active tab per scope and the title counter. */
export type TabsState = {
  tabs: readonly QueryTab[];
  active: Readonly<Record<string, string | undefined>>;
  next: number;
  /** Descriptor-only split layouts; no editor instances or results. */
  splits?: SplitStates;
  /** Tab groups by id; a group without tabs is ignored and not stored. */
  groups?: Readonly<Record<string, TabGroup>>;
  /** A window tab (the connection flow) shown in place of the scope's tab. */
  current?: string;
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

function kindOf(init: QueryTabInit): QueryTab['kind'] {
  if (init.flow) return 'connection';
  if (init.monitor) return 'monitor';
  if (init.inspection) return 'inspection';
  if (init.table) return 'table';
  return init.processes ? 'processes' : 'query';
}

/** The window's current tab after `id` is focused: only window tabs stay current. */
function currentAfter(state: TabsState, tab: QueryTab): TabsState {
  const next = { ...state };
  if (tab.kind === 'connection') next.current = tab.id;
  else delete next.current;
  return next;
}

/** The scope's open connection tab for the same flow, if any. */
function openFlow(state: TabsState, init: QueryTabInit): QueryTab | undefined {
  return state.tabs.find(
    (tab) =>
      tab.kind === 'connection' &&
      tab.flow?.mode === init.flow?.mode &&
      tab.flow?.profileId === init.flow?.profileId &&
      tab.flow?.filePath === init.flow?.filePath,
  );
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
  if (kind === 'processes' || kind === 'monitor') {
    const existing = tabsOf(state, scope).find((tab) => tab.kind === kind);
    if (existing) return selectTab(state, existing.id);
  }
  const flow = kind === 'connection' ? openFlow(state, init) : undefined;
  if (flow) return selectTab(state, flow.id);
  const tab: QueryTab = {
    id: `${kind}-${String(state.next)}`,
    kind,
    title: init.title ?? t('sqlQueryNext', { next: String(state.next) }),
    connectionId: scope.connectionId,
    database: scope.database,
    sql: init.sql ?? '',
    ...(init.objectPath ? { objectPath: init.objectPath } : {}),
    ...(init.table ? { table: init.table } : {}),
    ...(init.inspection ? { inspection: init.inspection } : {}),
    ...(init.file ? { file: init.file } : {}),
    ...(init.preview ? { preview: true } : {}),
    ...(init.flow ? { flow: init.flow } : {}),
  };
  return {
    ...currentAfter(state, tab),
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
  const { current, ...rest } = state;
  return {
    ...rest,
    ...(current && current !== id ? { current } : {}),
    tabs: state.tabs.filter((tab) => tab.id !== id),
    active: { ...state.active, [key]: active },
  };
}

/** Focus a tab within its scope. */
export function selectTab(state: TabsState, id: string): TabsState {
  const tab = state.tabs.find((item) => item.id === id);
  if (!tab) return state;
  return {
    ...currentAfter(state, tab),
    active: { ...state.active, [scopeKey(tab)]: id },
  };
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
