// SPDX-License-Identifier: Apache-2.0
import { generalSettings } from '../settings/general';
import {
  loadWorkspaceState,
  saveWorkspaceState,
} from '../settings/workspaceState';
import { emptyTabs, scopeKey } from './tabs';
import type { QueryTab, TabFile, TableObject, TabsState } from './tabs';

/**
 * Open tabs as stored: query tabs with their text, file and parameters (only
 * with `withText`, i.e. Auto Save), table tabs with their relation.
 */
export function serializeTabs(state: TabsState, withText = true): string {
  const tabs = state.tabs.flatMap((tab) => {
    if (tab.kind === 'table') return [tab];
    return withText ? [tab] : [];
  });
  return JSON.stringify({ tabs, active: state.active, next: state.next });
}

const isString = (value: unknown): value is string => typeof value === 'string';
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

function fileOf(value: unknown): TabFile | undefined {
  if (!isRecord(value)) return undefined;
  const { path, name, saved, modifiedMs } = value;
  if (!isString(path) || !isString(name) || !isString(saved)) return undefined;
  return typeof modifiedMs === 'number'
    ? { path, name, saved, modifiedMs }
    : { path, name, saved };
}

function paramsOf(value: unknown): Record<string, string> | undefined {
  if (!isRecord(value)) return undefined;
  const entries = Object.entries(value).filter(([, item]) => isString(item));
  return Object.fromEntries(entries) as Record<string, string>;
}

function tableOf(value: unknown): TableObject | undefined {
  if (!isRecord(value)) return undefined;
  const { database, schema, name } = value;
  if (!isString(database) || !isString(schema) || !isString(name))
    return undefined;
  return { database, schema, name };
}

/** The kind-specific part of a stored tab; `null` when unusable. */
function contentOf(
  value: Record<string, unknown>,
): Pick<QueryTab, 'kind' | 'table' | 'objectPath'> | null {
  if (value.kind === 'query') return { kind: 'query' };
  const table = value.kind === 'table' ? tableOf(value.table) : undefined;
  if (!table) return null;
  return { kind: 'table', table, objectPath: [table.schema, table.name] };
}

/** A stored tab, or `null` when anything essential is missing. */
function tabOf(value: unknown): QueryTab | null {
  if (!isRecord(value)) return null;
  const { id, connectionId, database, title, sql } = value;
  const content = contentOf(value);
  if (!content || ![id, connectionId, database, title, sql].every(isString))
    return null;
  const file = fileOf(value.file);
  const params = paramsOf(value.params);
  return {
    id: id as string,
    connectionId: connectionId as string,
    database: database as string,
    title: title as string,
    sql: sql as string,
    ...content,
    ...(file ? { file } : {}),
    ...(params ? { params } : {}),
    ...(value.pinned === true ? { pinned: true } : {}),
    ...(value.preview === true ? { preview: true } : {}),
  };
}

/** Each scope's stored active tab, or its last tab when that one is gone. */
function activeOf(
  tabs: readonly QueryTab[],
  stored: unknown,
): TabsState['active'] {
  const active: Record<string, string> = {};
  for (const tab of tabs) active[scopeKey(tab)] = tab.id;
  if (!isRecord(stored)) return active;
  for (const [key, id] of Object.entries(stored)) {
    const known = tabs.some((tab) => tab.id === id && scopeKey(tab) === key);
    if (known && isString(id)) active[key] = id;
  }
  return active;
}

/** Restore stored query tabs; anything unreadable starts empty. */
export function restoreTabs(text: string | null): TabsState {
  let stored: unknown;
  try {
    stored = text ? JSON.parse(text) : null;
  } catch {
    return emptyTabs;
  }
  if (!isRecord(stored) || !Array.isArray(stored.tabs)) return emptyTabs;
  const tabs = stored.tabs.map(tabOf).filter((tab) => tab !== null);
  const numbers = tabs.map((tab) => Number(tab.id.split('-').at(-1)) || 0);
  const next = Math.max(
    typeof stored.next === 'number' ? stored.next : 1,
    ...numbers.map((number) => number + 1),
  );
  return { tabs, active: activeOf(tabs, stored.active), next };
}

/**
 * The tabs saved in the local store (imported once from web storage); none
 * when session restore is off.
 */
export async function loadTabs(): Promise<TabsState> {
  const text = await loadWorkspaceState('queryTabs');
  if (!generalSettings.get().restoreSession) return emptyTabs;
  return restoreTabs(text);
}

/**
 * Store the tabs for the next launch in the background: query text only
 * with Auto Save, and nothing when session restore is off.
 */
export function writeTabs(
  state: TabsState,
  options: { autoSave: boolean; restore: boolean },
): void {
  saveWorkspaceState(
    'queryTabs',
    options.restore ? serializeTabs(state, options.autoSave) : null,
  );
}
