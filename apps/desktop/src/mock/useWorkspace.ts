// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { connections } from './data/connections';
import { databaseFor, databases } from './data/databases';
import type { DatabaseObject } from './data/databases';
import {
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
const initialSessions = (): Session[] =>
  connections.map((connection, index) => ({
    active: connection.database,
    opened: [connection.database],
    documents: {
      [connection.database]: initialDocuments(index, connection.database),
    },
  }));
/** UI-only workspace → database state; tab sets survive database and workspace switches. */
export function useWorkspace(): ReturnType<typeof useWorkspaceValue> {
  const [connection, setConnection] = useState(0);
  const [sessions, setSessions] = useState(initialSessions);
  const session = sessions[connection] ?? initialSessions()[0];
  const database = session?.active ?? 'orders';
  const document =
    session?.documents[database] ?? initialDocuments(connection, database);
  const update = useCallback(
    (change: (current: Documents) => Documents) => {
      setSessions((current) =>
        current.map((item, index) =>
          index !== connection
            ? item
            : {
                ...item,
                documents: {
                  ...item.documents,
                  [item.active]: change(
                    item.documents[item.active] ??
                      initialDocuments(index, item.active),
                  ),
                },
              },
        ),
      );
    },
    [connection],
  );
  const selectConnection = useCallback((index: number) => {
    if (connections[index]) setConnection(index);
  }, []);
  const openDatabase = (name: string): void => {
    if (!databases[connection]?.some((item) => item.name === name)) return;
    setSessions((current) =>
      current.map((item, index) =>
        index !== connection
          ? item
          : {
              ...item,
              active: name,
              opened: item.opened.includes(name)
                ? item.opened
                : [...item.opened, name],
              documents: {
                ...item.documents,
                [name]: item.documents[name] ?? initialDocuments(index, name),
              },
            },
      ),
    );
  };
  const closeDatabase = (name: string): void => {
    setSessions((current) =>
      current.map((item, index) => {
        if (index !== connection || item.opened.length < 2) return item;
        const opened = item.opened.filter((database) => database !== name);
        return {
          ...item,
          opened,
          active:
            item.active === name ? (opened[0] ?? item.active) : item.active,
        };
      }),
    );
  };
  return useWorkspaceValue({
    connection,
    database,
    document,
    session,
    update,
    selectConnection,
    openDatabase,
    closeDatabase,
  });
}
function useWorkspaceValue(input: {
  connection: number;
  database: string;
  document: Documents;
  session: Session | undefined;
  update: (change: (current: Documents) => Documents) => void;
  selectConnection: (index: number) => void;
  openDatabase: (name: string) => void;
  closeDatabase: (name: string) => void;
}): {
  connection: number;
  database: string;
  catalog: ReturnType<typeof databaseFor>;
  tabs: Documents['tabs'];
  active: string;
  table: string;
  schema: string;
  opened: readonly string[];
  selectConnection: (index: number) => void;
  openDatabase: (name: string) => void;
  closeDatabase: (name: string) => void;
  setSchema: (schema: string) => void;
  selectTab: (id: string) => void;
  addTab: (sql?: string) => void;
  closeTab: (id: string) => void;
  openTable: (object: DatabaseObject) => void;
} {
  const { document, update } = input;
  const tab = document.tabs.find((tab) => tab.id === document.active);
  return {
    connection: input.connection,
    database: input.database,
    catalog: databaseFor(input.connection, input.database),
    ...document,
    table: tab?.label ?? '',
    opened: input.session?.opened ?? [],
    selectConnection: input.selectConnection,
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
    closeTab: (id) => {
      update((state) => closeDocument(state, id));
    },
    openTable: (object) => {
      update((state) => tableDocument(state, object));
    },
  };
}
