// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';
import type { SessionStatus } from './sessions';

/** One connection of the last session: its opened databases and the active one. */
export type SavedConnection = {
  id: string;
  databases: readonly string[];
  database: string;
};

/** The open connections and the active one, for session restore. */
export type SavedSession = {
  active: string | null;
  connections: readonly SavedConnection[];
};

const emptySession: SavedSession = { active: null, connections: [] };

const isString = (value: unknown): value is string => typeof value === 'string';

function connectionOf(value: unknown): SavedConnection | null {
  if (typeof value !== 'object' || value === null) return null;
  const { id, databases, database } = value as Record<string, unknown>;
  if (!isString(id) || !isString(database) || !Array.isArray(databases))
    return null;
  return { id, database, databases: databases.filter(isString) };
}

/** Parse a stored session; anything unreadable is an empty session. */
export function parseSession(text: string | null): SavedSession {
  let stored: unknown;
  try {
    stored = text ? JSON.parse(text) : null;
  } catch {
    return emptySession;
  }
  if (typeof stored !== 'object' || stored === null) return emptySession;
  const { active, connections } = stored as Record<string, unknown>;
  return {
    active: isString(active) ? active : null,
    connections: Array.isArray(connections)
      ? connections.map(connectionOf).filter((item) => item !== null)
      : [],
  };
}

/**
 * Connections that must not connect without the user: production ones and
 * those that confirm before writing.
 */
export function needsConsent(profile: ProfileDto): boolean {
  return profile.env.kind === 'prod' || profile.safety.confirmWrites;
}

/** Open (or opening) databases of a connection, in opening order. */
function openedNames(status: SessionStatus): string[] {
  if (status.state !== 'connected') return [];
  return status.databases
    .filter((item) => item.state !== 'failed')
    .map((item) => item.name);
}

/** The current session: live connections plus restored ones still waiting. */
export function sessionOf(input: {
  active: string | null;
  profiles: readonly ProfileDto[];
  statusOf: (id: string) => SessionStatus;
  databaseOf: (id: string) => string;
  waiting: ReadonlyMap<string, SavedConnection>;
}): SavedSession {
  const connections = input.profiles.flatMap((profile) => {
    const status = input.statusOf(profile.id);
    if (status.state === 'connected')
      return [
        {
          id: profile.id,
          databases: openedNames(status),
          database: input.databaseOf(profile.id),
        },
      ];
    const waiting = input.waiting.get(profile.id);
    return waiting ? [waiting] : [];
  });
  return { active: input.active, connections };
}
