// SPDX-License-Identifier: Apache-2.0
import { saveWorkspaceState } from '../settings/workspaceState';
import { inspectionOf } from './inspectionDescriptor';
import { parseSplits } from './splitState';
import { emptyTabs, scopeKey } from './tabs';
import type { QueryTab, TabFile, TableObject, TabsState } from './tabs';

/**
 * Open tabs as stored: query tabs with their text and file (only
 * with `withText`, i.e. Auto Save), table and inspector identities and split layout.
 */
export function serializeTabs(state: TabsState, withText = true): string {
  const tabs = state.tabs.flatMap((tab) => {
    if (tab.kind === 'processes') return [];
    if (tab.kind === 'query' && !withText) return [];
    // Allowlist descriptors: parameter values, results and transient file errors never persist.
    const { id, kind, connectionId, database, title, sql, pinned, preview } =
      tab;
    const table = tableOf(tab.table);
    const inspection = inspectionOf(tab.inspection);
    const file = tab.file
      ? {
          path: tab.file.path,
          name: tab.file.name,
          saved: tab.file.saved,
          modifiedMs: tab.file.modifiedMs,
        }
      : undefined;
    return [
      {
        id,
        kind,
        connectionId,
        database,
        title,
        sql,
        pinned,
        preview,
        table,
        inspection,
        file,
      },
    ];
  });
  return JSON.stringify({
    version: 1,
    tabs,
    active: activeOf(tabs, state.active),
    next: state.next,
    splits: state.splits ? parseSplits(state.splits, tabs) : undefined,
  });
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

function tableOf(value: unknown): TableObject | undefined {
  if (!isRecord(value)) return undefined;
  const { database, schema, name } = value;
  if (!isString(database) || !isString(schema) || !isString(name))
    return undefined;
  const kinds = ['foreignTable', 'view', 'materializedView'];
  const kind = kinds.includes(String(value.kind))
    ? (value.kind as TableObject['kind'])
    : undefined;
  return { database, schema, name, ...(kind ? { kind } : {}) };
}

/** The kind-specific part of a stored tab; `null` when unusable. */
function contentOf(
  value: Record<string, unknown>,
): Pick<QueryTab, 'kind' | 'table' | 'objectPath' | 'inspection'> | null {
  const inspection =
    value.kind === 'inspection' ? inspectionOf(value.inspection) : undefined;
  if (inspection) return { kind: 'inspection', inspection };
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

  return {
    id: id as string,
    connectionId: connectionId as string,
    database: database as string,
    title: title as string,
    sql: sql as string,
    ...content,
    ...(file ? { file } : {}),
    ...(value.pinned === true ? { pinned: true } : {}),
    ...(value.preview === true ? { preview: true } : {}),
  };
}

/** Each scope's stored active tab, or its last tab when that one is gone. */
function activeOf(
  tabs: readonly Pick<QueryTab, 'id' | 'connectionId' | 'database'>[],
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
  if (stored.version !== undefined && stored.version !== 1) return emptyTabs;
  const tabs = stored.tabs.map(tabOf).filter((tab) => tab !== null);
  const numbers = tabs
    .map((tab) => Number(tab.id.split('-').at(-1)))
    .filter(Number.isSafeInteger);
  const next = Math.max(
    typeof stored.next === 'number' && Number.isSafeInteger(stored.next)
      ? Math.max(1, stored.next)
      : 1,
    ...numbers.map((number) => number + 1),
  );
  return {
    tabs,
    active: activeOf(tabs, stored.active),
    next,
    ...(stored.splits ? { splits: parseSplits(stored.splits, tabs) } : {}),
  };
}

/**
 * Store the tabs for the next launch in the background: query text only
 * with Auto Save. Disabling automatic restore still permits explicit reopening.
 */
export function writeTabs(
  state: TabsState,
  options: { autoSave: boolean; restore: boolean },
): void {
  saveWorkspaceState('queryTabs', serializeTabs(state, options.autoSave));
}
