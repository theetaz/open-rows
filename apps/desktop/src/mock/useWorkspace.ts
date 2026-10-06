// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { connections } from './data/connections';
import { initialTabs } from './data/content';
import type { WorkspaceTab } from './data/content';

type Documents = {
  tabs: readonly WorkspaceTab[];
  active: string;
  next: number;
};
type WorkspaceState = Documents & {
  connection: number;
  schema: string;
  setSchema: (schema: string) => void;
  selectConnection: (index: number) => void;
  selectTab: (id: string) => void;
  addTab: (sql?: string) => void;
  closeTab: (id: string) => void;
  openTable: (label: string) => void;
};
/** UI-only connection and document state; never opens a connection. */
export function useWorkspace(): WorkspaceState {
  const [connection, setConnection] = useState(0);
  const [schema, setSchema] = useState('public');
  const [documents, setDocuments] = useState<Documents>({
    tabs: initialTabs,
    active: 'orders',
    next: 1,
  });
  const selectTab = useCallback((id: string) => {
    setDocuments((state) => ({ ...state, active: id }));
  }, []);
  const addTab = useCallback((sql?: string) => {
    setDocuments((state) => newQuery(state, sql));
  }, []);
  const closeTab = useCallback((id: string) => {
    setDocuments((state) => closeDocument(state, id));
  }, []);
  const openTable = useCallback((label: string) => {
    setDocuments((state) => tableDocument(state, label));
  }, []);
  const selectConnection = useCallback((index: number) => {
    const selected = connections[index];
    if (!selected) return;
    setConnection(index);
    setSchema(selected.schemas[0] ?? 'public');
    setDocuments((state) => ({
      ...state,
      tabs: selected.tables.map(([label]) => ({
        id: label,
        label,
        kind: 'table',
      })),
      active: selected.tables[0]?.[0] ?? '',
    }));
  }, []);
  return {
    connection,
    schema,
    setSchema,
    ...documents,
    selectTab,
    addTab,
    closeTab,
    openTable,
    selectConnection,
  };
}
function newQuery(state: Documents, sql?: string): Documents {
  const tab: WorkspaceTab = {
    id: `sql-${String(state.next)}`,
    label: `SQL Query ${String(state.next)}`,
    kind: 'query',
    ...(sql ? { sql } : {}),
  };
  return { tabs: [...state.tabs, tab], active: tab.id, next: state.next + 1 };
}
function closeDocument(state: Documents, id: string): Documents {
  const index = state.tabs.findIndex((tab) => tab.id === id);
  const tabs = state.tabs.filter((tab) => tab.id !== id);
  return {
    ...state,
    tabs,
    active:
      state.active === id
        ? (tabs[Math.max(0, index - 1)]?.id ?? '')
        : state.active,
  };
}
function tableDocument(state: Documents, label: string): Documents {
  return {
    ...state,
    active: label,
    tabs: state.tabs.some((tab) => tab.id === label)
      ? state.tabs
      : [...state.tabs, { id: label, label, kind: 'table' }],
  };
}
