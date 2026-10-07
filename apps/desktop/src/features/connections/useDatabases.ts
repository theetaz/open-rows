// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { disconnected } from './sessions';
import type { OpenedDatabase } from './sessions';
import type { Sessions } from './useSessions';

/** Opened databases of the active connection and which one is active. */
export type Databases = {
  /** Opened databases of the active connection, in opening order. */
  databases: readonly OpenedDatabase[];
  /** Active database of the active connection; empty when none is open. */
  database: string;
  /** Open `name` on the active connection (if needed) and make it active. */
  openDatabase: (name: string) => void;
  /** Close `name` and its sessions; a neighbour becomes active. */
  closeDatabase: (name: string) => void;
};

/** The database next to `name` that becomes active when `name` closes. */
export function neighbour(
  databases: readonly OpenedDatabase[],
  name: string,
): string | undefined {
  const index = databases.findIndex((item) => item.name === name);
  return (databases[index - 1] ?? databases[index + 1])?.name;
}

/** Active database per saved connection; survives switching connections. */
export function useDatabases(
  sessions: Sessions,
  profileId: string | undefined,
): Databases {
  const [chosen, setChosen] = useState<
    Readonly<Record<string, string | undefined>>
  >({});
  const status = profileId ? sessions.statusOf(profileId) : disconnected;
  const databases = status.state === 'connected' ? status.databases : [];
  const wanted = profileId ? chosen[profileId] : undefined;
  const database =
    (databases.some((item) => item.name === wanted) ? wanted : undefined) ??
    databases[0]?.name ??
    '';
  const choose = (id: string, name: string | undefined): void => {
    setChosen((current) => ({ ...current, [id]: name }));
  };
  return {
    databases,
    database,
    openDatabase: (name) => {
      if (!profileId) return;
      choose(profileId, name);
      void sessions.openDatabase(profileId, name);
    },
    closeDatabase: (name) => {
      if (!profileId) return;
      if (database === name) choose(profileId, neighbour(databases, name));
      void sessions.closeDatabase(profileId, name);
    },
  };
}
