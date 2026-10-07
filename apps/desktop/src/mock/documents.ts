// SPDX-License-Identifier: Apache-2.0
import { initialTabs } from './data/content';
import type { WorkspaceTab } from './data/content';
import type { ObjectRef } from '../features/schema';
export type Documents = {
  tabs: readonly WorkspaceTab[];
  active: string;
  next: number;
  schema: string;
};
/** Sample tabs for a connection's database, created only on its first opening (until D-07). */
export function initialDocuments(): Documents {
  const tabs = initialTabs.map((tab) => ({ ...tab, schema: 'public' }));
  return { tabs, active: tabs[0]?.id ?? '', next: 1, schema: 'public' };
}
/** Add a query descriptor to the current database's tabs. */
export function newQuery(state: Documents, sql?: string): Documents {
  const tab: WorkspaceTab = {
    id: `sql-${String(state.next)}`,
    label: `SQL Query ${String(state.next)}`,
    kind: 'query',
    schema: state.schema,
    ...(sql ? { sql } : {}),
  };
  return {
    ...state,
    tabs: [...state.tabs, tab],
    active: tab.id,
    next: state.next + 1,
  };
}
/** Close one descriptor and focus its nearest remaining neighbour. */
export function closeDocument(state: Documents, id: string): Documents {
  const index = state.tabs.findIndex((tab) => tab.id === id);
  const tabs = state.tabs.filter((tab) => tab.id !== id);
  const active =
    state.active === id
      ? (tabs[Math.max(0, index - 1)]?.id ?? '')
      : state.active;
  return {
    ...state,
    tabs,
    active,
    schema: tabs.find((tab) => tab.id === active)?.schema ?? state.schema,
  };
}
/**
 * Focus an existing object descriptor or append a schema-qualified object tab
 * for a real catalog path (the placeholder content stays until D-07/D-08).
 */
export function tableDocument(state: Documents, object: ObjectRef): Documents {
  const label =
    object.arguments === null
      ? object.name
      : `${object.name}(${object.arguments})`;
  const existing = state.tabs.find(
    (tab) => tab.label === label && tab.schema === object.schema,
  );
  const id =
    existing?.id ??
    (state.tabs.some((tab) => tab.id === label)
      ? `${object.schema}.${label}`
      : label);
  return {
    ...state,
    schema: object.schema,
    active: id,
    tabs: existing
      ? state.tabs
      : [...state.tabs, { id, label, kind: 'table', schema: object.schema }],
  };
}

/** Retain editable state in descriptors while inactive content is unmounted. */
export function editDocument(
  state: Documents,
  fields: Partial<Pick<WorkspaceTab, 'sql' | 'tableState'>>,
): Documents {
  return {
    ...state,
    tabs: state.tabs.map((item) =>
      item.id === state.active ? { ...item, ...fields } : item,
    ),
  };
}
