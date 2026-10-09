// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import { generalSettings } from '../settings/general';
import {
  loadWorkspaceState,
  saveWorkspaceState,
} from '../settings/workspaceState';
import { parseSession, sessionOf } from './sessionState';
import type { SavedConnection, SavedSession } from './sessionState';
import type { Databases } from './useDatabases';
import type { Sessions } from './useSessions';

type Input = {
  profiles: readonly ProfileDto[];
  activeId: string | null;
  setActiveId: (id: string) => void;
  sessions: Sessions;
  databases: Pick<Databases, 'databaseOf' | 'prefer'>;
};

/** The last session from the local store (imported once from web storage). */
async function readSession(): Promise<SavedSession | null> {
  try {
    return parseSession(await loadWorkspaceState('session'));
  } catch {
    return null;
  }
}

/** Remember the session in the background; `null` forgets it. */
function writeSession(session: SavedSession | null): void {
  saveWorkspaceState('session', session ? JSON.stringify(session) : null);
}

/** Restore the saved session once profiles are known; resolves who still waits. */
function restore(
  input: Input,
  saved: SavedSession | null,
  explicit = false,
): Map<string, SavedConnection> {
  const waiting = new Map<string, SavedConnection>();
  if (!saved || (!explicit && !generalSettings.get().restoreSession))
    return waiting;
  for (const connection of saved.connections) {
    const profile = input.profiles.find((item) => item.id === connection.id);
    if (!profile) continue;
    waiting.set(profile.id, connection);
    if (connection.database)
      input.databases.prefer(profile.id, connection.database);
  }
  const active = saved.active;
  if (active && input.profiles.some((item) => item.id === active))
    input.setActiveId(active);
  return waiting;
}

/**
 * Session restore (M1-26): on launch reopen the previous connections, their
 * databases and the active connection; while running, remember them.
 */
export function useSessionRestore(
  input: Input,
  loaded: boolean,
): {
  databasesOf: (id: string) => readonly string[];
  forgetDatabase: (id: string, name: string) => void;
  rememberConnection: (id: string) => void;
} {
  const [waiting, setWaiting] = useState<Map<string, SavedConnection> | null>(
    null,
  );
  const latest = useRef(input);
  const started = useRef(false);
  const previous = useRef<SavedSession | null>(null);
  useEffect(() => {
    latest.current = input;
  });
  useEffect(() => {
    if (!loaded || started.current) return;
    started.current = true;
    void readSession()
      .then((saved) => {
        previous.current = saved;
        return restore(latest.current, saved);
      })
      .then(setWaiting);
  }, [loaded]);
  useEffect(() => {
    const reopen = (): void => {
      setWaiting(restore(latest.current, previous.current, true));
    };
    window.addEventListener('openrows:reopen-session', reopen);
    return () => {
      window.removeEventListener('openrows:reopen-session', reopen);
    };
  }, []);

  const session =
    waiting &&
    JSON.stringify(
      sessionOf({
        active: input.activeId,
        profiles: input.profiles,
        statusOf: input.sessions.statusOf,
        databaseOf: input.databases.databaseOf,
        waiting,
      }),
    );
  useEffect(() => {
    if (session === null) return;
    const saved = JSON.parse(session) as SavedSession;
    writeSession(saved);
  }, [session]);
  return {
    databasesOf: (id) => waiting?.get(id)?.databases ?? [],
    ...descriptorActions(input, setWaiting),
  };
}

/** Retain explicitly disconnected contexts and remove explicitly closed databases. */
function descriptorActions(
  input: Input,
  setWaiting: React.Dispatch<
    React.SetStateAction<Map<string, SavedConnection> | null>
  >,
): {
  rememberConnection: (id: string) => void;
  forgetDatabase: (id: string, name: string) => void;
} {
  return {
    rememberConnection: (id) => {
      const status = input.sessions.statusOf(id);
      if (status.state !== 'connected') return;
      setWaiting((current) =>
        new Map(current).set(id, {
          id,
          database: input.databases.databaseOf(id),
          databases: [
            ...new Set([
              ...(current?.get(id)?.databases ?? []),
              ...status.databases.map((database) => database.name),
            ]),
          ],
        }),
      );
    },
    forgetDatabase: (id, name) => {
      setWaiting((current) => {
        const next = new Map(current);
        const previous = next.get(id);
        if (previous)
          next.set(id, {
            ...previous,
            databases: previous.databases.filter(
              (database) => database !== name,
            ),
            database:
              previous.database === name
                ? (previous.databases.find((database) => database !== name) ??
                  '')
                : previous.database,
          });
        return next;
      });
    },
  };
}
