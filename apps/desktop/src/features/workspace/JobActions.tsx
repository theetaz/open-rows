// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import { TablesExportSheet } from '../export';
import type { TablesTarget } from '../export';
import { pickRunFileFor, RunFileHost } from '../runfile';

/** Background-job actions of the active connection. */
export type JobActions = {
  /** Run SQL File… on a database of a connection (the active one by default). */
  runFile: (database: string, profileId?: string) => void;
  /** Export Tables… of a schema of the active connection's database. */
  exportTables: (database: string, schema: string) => void;
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

/** Run SQL File… and Export Tables…, which start background jobs. */
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
    tables,
    closeTables: () => {
      setTables(null);
    },
  };
}

/** The run-file sheet and the tables export sheet when shown. */
export function JobSheets({
  actions,
}: {
  actions: JobActions;
}): React.JSX.Element {
  return (
    <>
      <RunFileHost />
      {actions.tables && (
        <TablesExportSheet {...actions.tables} onClose={actions.closeTables} />
      )}
    </>
  );
}
