// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { TableState } from './data/content';
import type { ObjectRef } from '../features/schema';
import {
  editDocument,
  initialDocuments,
  newQuery,
  closeDocument,
  tableDocument,
} from './documents';
import type { Documents } from './documents';
/**
 * Sample tab sets per connection and database (`scope`); they survive
 * database and connection switches. Replaced by real tabs in D-07.
 */
export function useWorkspace(
  scope: string,
): ReturnType<typeof useWorkspaceValue> {
  const [sets, setSets] = useState<Readonly<Record<string, Documents>>>({});
  const document = sets[scope] ?? initialDocuments();
  const update = useCallback(
    (change: (current: Documents) => Documents) => {
      setSets((current) => ({
        ...current,
        [scope]: change(current[scope] ?? initialDocuments()),
      }));
    },
    [scope],
  );
  return useWorkspaceValue(document, update);
}
function useWorkspaceValue(
  document: Documents,
  update: (change: (current: Documents) => Documents) => void,
): {
  tabs: Documents['tabs'];
  active: string;
  table: string;
  selectTab: (id: string) => void;
  addTab: (sql?: string) => void;
  closeTab: (id: string) => void;
  editSql: (sql: string) => void;
  editTable: (tableState: TableState) => void;
  openTable: (object: ObjectRef) => void;
} {
  const tab = document.tabs.find((tab) => tab.id === document.active);
  return {
    tabs: document.tabs,
    active: document.active,
    table: tab?.label ?? '',
    selectTab: (id) => {
      update((state) => ({ ...state, active: id }));
    },
    addTab: useCallback(
      (sql?: string) => {
        update((state) => newQuery(state, sql));
      },
      [update],
    ),
    editTable: (tableState) => {
      update((state) => editDocument(state, { tableState }));
    },
    editSql: (sql) => {
      update((state) => editDocument(state, { sql }));
    },
    closeTab: (id) => {
      update((state) => closeDocument(state, id));
    },
    openTable: (object) => {
      update((state) => tableDocument(state, object));
    },
  };
}
