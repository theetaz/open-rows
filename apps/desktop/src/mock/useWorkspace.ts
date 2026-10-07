// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { sampleCatalog } from './data/connections';
import { databaseFor, databases } from './data/databases';
import type { DatabaseObject } from './data/databases';
import type { TableState } from './data/content';
import {
  editDocument,
  initialDocuments,
  newQuery,
  closeDocument,
  tableDocument,
} from './documents';
import type { Documents } from './documents';
type Session = {
  active: string;
  opened: string[];
  documents: Record<string, Documents>;
};
const initialSession = (): Session => ({
  active: sampleCatalog.database,
  opened: [sampleCatalog.database],
  documents: {
    [sampleCatalog.database]: initialDocuments(sampleCatalog.database),
  },
});
/** UI-only sample database state; tab sets survive database switches. */
export function useWorkspace(): ReturnType<typeof useWorkspaceValue> {
  const [session, setSession] = useState(initialSession);
  const database = session.active;
  const document = session.documents[database] ?? initialDocuments(database);
  const update = useCallback((change: (current: Documents) => Documents) => {
    setSession((item) => ({
      ...item,
      documents: {
        ...item.documents,
        [item.active]: change(
          item.documents[item.active] ?? initialDocuments(item.active),
        ),
      },
    }));
  }, []);
  const openDatabase = (name: string): void => {
    if (!databases.some((item) => item.name === name)) return;
    setSession((item) => ({
      ...item,
      active: name,
      opened: item.opened.includes(name) ? item.opened : [...item.opened, name],
      documents: {
        ...item.documents,
        [name]: item.documents[name] ?? initialDocuments(name),
      },
    }));
  };
  const closeDatabase = (name: string): void => {
    setSession((item) => {
      if (item.opened.length < 2) return item;
      const opened = item.opened.filter((database) => database !== name);
      return {
        ...item,
        opened,
        active: item.active === name ? (opened[0] ?? item.active) : item.active,
      };
    });
  };
  return useWorkspaceValue({
    database,
    document,
    session,
    update,
    openDatabase,
    closeDatabase,
  });
}
function useWorkspaceValue(input: {
  database: string;
  document: Documents;
  session: Session;
  update: (change: (current: Documents) => Documents) => void;
  openDatabase: (name: string) => void;
  closeDatabase: (name: string) => void;
}): {
  database: string;
  catalog: ReturnType<typeof databaseFor>;
  tabs: Documents['tabs'];
  active: string;
  table: string;
  schema: string;
  opened: readonly string[];
  openDatabase: (name: string) => void;
  closeDatabase: (name: string) => void;
  setSchema: (schema: string) => void;
  selectTab: (id: string) => void;
  addTab: (sql?: string) => void;
  closeTab: (id: string) => void;
  editSql: (sql: string) => void;
  editTable: (tableState: TableState) => void;
  openTable: (object: DatabaseObject) => void;
} {
  const { document, update } = input;
  const tab = document.tabs.find((tab) => tab.id === document.active);
  return {
    database: input.database,
    catalog: databaseFor(input.database),
    ...document,
    table: tab?.label ?? '',
    opened: input.session.opened,
    openDatabase: input.openDatabase,
    closeDatabase: input.closeDatabase,
    setSchema: (schema) => {
      update((state) => ({ ...state, schema }));
    },
    selectTab: (id) => {
      update((state) => ({
        ...state,
        active: id,
        schema: state.tabs.find((tab) => tab.id === id)?.schema ?? state.schema,
      }));
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
