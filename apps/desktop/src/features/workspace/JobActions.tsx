// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import { DumpHost, openBackup, pickRestoreFor } from '../dump';
import type { DumpTarget } from '../dump';
import { TablesExportSheet } from '../export';
import type { TablesTarget } from '../export';
import { pickRunFileFor, RunFileHost } from '../runfile';

/** Background-job actions of the active connection. */
export type JobActions = {
  /** Run SQL File… on a database of a connection (the active one by default). */
  runFile: (database: string, profileId?: string) => void;
  /** Export Tables… of a schema of the active connection's database. */
  exportTables: (database: string, schema: string) => void;
  /** Back Up Database… of a database of the active connection. */
  backup: (database: string) => void;
  /** Restore Database… into a database of the active connection. */
  restore: (database: string) => void;
  tables: TablesTarget | null;
  closeTables: () => void;
};

/** The open session of `database` on a connection, with its names. */
function target(
  connections: Connections,
  database: string,
  profileId = connections.active?.id,
): { sessionId: string; profileId: string; connection: string } | null {
  const profile = connections.profiles.find((item) => item.id === profileId);
  if (!profile || !database) return null;
  const sessionId = sessionOf(connections.statusOf(profile.id), database);
  return sessionId
    ? { sessionId, profileId: profile.id, connection: profile.name }
    : null;
}

/** The open database a backup or restore acts on, with its server. */
function dumpTarget(
  connections: Connections,
  database: string,
): DumpTarget | null {
  const found = target(connections, database);
  if (!found) return null;
  const status = connections.statusOf(found.profileId);
  const server = status.state === 'connected' ? status.server : null;
  return {
    ...found,
    database,
    serverProduct: server?.product ?? null,
    serverVersion: server?.serverVersion ?? null,
  };
}

/** Run SQL File…, Export Tables…, Back Up and Restore, which start background jobs. */
export function useJobActions(connections: Connections): JobActions {
  const [tables, setTables] = useState<TablesTarget | null>(null);
  return {
    runFile: (database, profileId) => {
      const found = target(connections, database, profileId);
      if (!found) return;
      // A failed panel has nothing to show; the menu item stays usable.
      pickRunFileFor({ ...found, database }).catch(() => undefined);
    },
    exportTables: (database, schema) => {
      const found = target(connections, database);
      if (found && schema) setTables({ ...found, database, schema });
    },
    backup: (database) => {
      const found = dumpTarget(connections, database);
      if (found) openBackup(found);
    },
    restore: (database) => {
      const found = dumpTarget(connections, database);
      // A failed panel has nothing to show; the menu item stays usable.
      if (found) pickRestoreFor(found).catch(() => undefined);
    },
    tables,
    closeTables: () => {
      setTables(null);
    },
  };
}

/** The run-file, dump and tables export sheets when shown. */
export function JobSheets({
  actions,
}: {
  actions: JobActions;
}): React.JSX.Element {
  return (
    <>
      <RunFileHost />
      <DumpHost />
      {actions.tables && (
        <TablesExportSheet {...actions.tables} onClose={actions.closeTables} />
      )}
    </>
  );
}
