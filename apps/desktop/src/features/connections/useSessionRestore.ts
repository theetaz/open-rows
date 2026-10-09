// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import { generalSettings } from '../settings/general';
import {
  SESSION_KEY,
  needsConsent,
  parseSession,
  sessionOf,
} from './sessionState';
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

function readSession(): SavedSession | null {
  try {
    return parseSession(localStorage.getItem(SESSION_KEY));
  } catch {
    return null;
  }
}

function writeSession(session: SavedSession | null): void {
  try {
    if (session) localStorage.setItem(SESSION_KEY, JSON.stringify(session));
    else localStorage.removeItem(SESSION_KEY);
  } catch {
    /* Without storage the next launch starts empty. */
  }
}

/** Reconnect one saved connection and reopen its databases. */
async function reopen(saved: SavedConnection, input: Input): Promise<void> {
  const home = await input.sessions.connect(saved.id);
  for (const name of saved.databases)
    if (name !== home) await input.sessions.openDatabase(saved.id, name);
}

/** Restore the saved session once profiles are known; returns who still waits. */
function restore(input: Input): Map<string, SavedConnection> {
  const waiting = new Map<string, SavedConnection>();
  const saved = readSession();
  if (!saved || !generalSettings.get().restoreSession) return waiting;
  for (const connection of saved.connections) {
    const profile = input.profiles.find((item) => item.id === connection.id);
    if (!profile) continue;
    waiting.set(profile.id, connection);
    if (connection.database)
      input.databases.prefer(profile.id, connection.database);
    // Production and confirm-before-write connections wait for the user.
    if (!needsConsent(profile)) void reopen(connection, input);
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
export function useSessionRestore(input: Input, loaded: boolean): void {
  const [waiting, setWaiting] = useState<Map<string, SavedConnection> | null>(
    null,
  );
  const latest = useRef(input);
  useEffect(() => {
    latest.current = input;
  });
  useEffect(() => {
    if (loaded && waiting === null) setWaiting(restore(latest.current));
  }, [loaded, waiting]);
  useForgetConnected(waiting, setWaiting, input.sessions);
  const { restoreSession } = generalSettings.use();
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
    writeSession(restoreSession ? (JSON.parse(session) as SavedSession) : null);
  }, [session, restoreSession]);
}

/** Restored connections stop waiting once connected (a later disconnect forgets them). */
function useForgetConnected(
  waiting: Map<string, SavedConnection> | null,
  setWaiting: (next: Map<string, SavedConnection>) => void,
  sessions: Sessions,
): void {
  const connected = waiting
    ? [...waiting.keys()].filter(
        (id) => sessions.statusOf(id).state === 'connected',
      )
    : [];
  const key = connected.join('\n');
  useEffect(() => {
    if (!waiting || !key) return;
    const done = new Set(key.split('\n'));
    setWaiting(new Map([...waiting].filter(([id]) => !done.has(id))));
  }, [key, waiting, setWaiting]);
}
