// SPDX-License-Identifier: Apache-2.0
import { emptyTabs, scopeKey } from './tabs';
import type { QueryTab, TabFile, TabsState } from './tabs';

/** Where query tabs persist on this device while Auto Save is on. */
export const TABS_KEY = 'openrows.queryTabs';

/** Query tabs (with their text, file and parameters) as stored; table tabs reopen from the sidebar. */
export function serializeTabs(state: TabsState): string {
  const tabs = state.tabs.filter((tab) => tab.kind === 'query');
  return JSON.stringify({ tabs, active: state.active, next: state.next });
}

const isString = (value: unknown): value is string => typeof value === 'string';
const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

function fileOf(value: unknown): TabFile | undefined {
  if (!isRecord(value)) return undefined;
  const { path, name, saved } = value;
  if (!isString(path) || !isString(name) || !isString(saved)) return undefined;
  return { path, name, saved };
}

function paramsOf(value: unknown): Record<string, string> | undefined {
  if (!isRecord(value)) return undefined;
  const entries = Object.entries(value).filter(([, item]) => isString(item));
  return Object.fromEntries(entries) as Record<string, string>;
}

/** A stored query tab, or `null` when anything essential is missing. */
function tabOf(value: unknown): QueryTab | null {
  if (!isRecord(value)) return null;
  const { id, connectionId, database, title, sql } = value;
  const fields = [id, connectionId, database, title, sql];
  if (value.kind !== 'query' || !fields.every(isString)) return null;
  const file = fileOf(value.file);
  const params = paramsOf(value.params);
  return {
    id: id as string,
    kind: 'query',
    connectionId: connectionId as string,
    database: database as string,
    title: title as string,
    sql: sql as string,
    ...(file ? { file } : {}),
    ...(params ? { params } : {}),
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

/** The stored tabs of this device. */
export function readTabs(): TabsState {
  try {
    return restoreTabs(localStorage.getItem(TABS_KEY));
  } catch {
    return emptyTabs;
  }
}

/** Store the tabs, or forget them when Auto Save is off. */
export function writeTabs(state: TabsState, autoSave: boolean): void {
  try {
    if (autoSave) localStorage.setItem(TABS_KEY, serializeTabs(state));
    else localStorage.removeItem(TABS_KEY);
  } catch {
    /* Without storage, tabs last for this session only. */
  }
}
