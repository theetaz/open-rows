// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import {
  closeConnection,
  closeSession,
  onSessionState,
  openConnection,
  openSession,
} from '@openrows/ipc';
import {
  applySessionEvent,
  disconnected,
  removeDatabase,
  setDatabase,
} from './sessions';
import type { SessionStatus, Statuses } from './sessions';

export type Sessions = {
  statusOf: (profileId: string) => SessionStatus;
  /**
   * Reach the server, then open the profile's database when it has one;
   * resolves that database's name (`null` when none or on failure).
   */
  connect: (profileId: string) => Promise<string | null>;
  disconnect: (profileId: string) => Promise<void>;
  /** Open a session on `database`; already open databases are kept. */
  openDatabase: (profileId: string, database: string) => Promise<void>;
  closeDatabase: (profileId: string, database: string) => Promise<void>;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

type Update = (change: (current: Statuses) => Statuses) => void;

/** Connections and their per-database sessions, synced with core events. */
export function useSessions(): Sessions {
  const [statuses, setStatuses] = useState<Statuses>({});
  const generation = useRef(0);
  useSessionEvents(setStatuses);
  const statusOf = (profileId: string): SessionStatus =>
    statuses[profileId] ?? disconnected;
  const set = (profileId: string, status: SessionStatus): void => {
    setStatuses((now) => ({ ...now, [profileId]: status }));
  };
  const openDatabase = async (profileId: string, database: string) => {
    const status = statuses[profileId];
    const known =
      status?.state === 'connected' &&
      status.databases.some(
        (item) => item.name === database && item.state !== 'failed',
      );
    if (!known) await openIn(setStatuses, profileId, database);
  };
  const connect = async (profileId: string): Promise<string | null> => {
    set(profileId, { state: 'connecting' });
    try {
      const opened = await openConnection(profileId);
      generation.current += 1;
      set(profileId, {
        state: 'connected',
        server: opened.server,
        insecure: opened.insecure,
        databases: [],
        generation: generation.current,
      });
      if (opened.database)
        await openIn(setStatuses, profileId, opened.database);
      return opened.database;
    } catch (error) {
      set(profileId, { state: 'failed', message: message(error) });
      return null;
    }
  };
  const disconnect = async (profileId: string): Promise<void> => {
    if (statuses[profileId]?.state !== 'connected') {
      set(profileId, disconnected);
      return;
    }
    try {
      await closeConnection(profileId);
      set(profileId, disconnected);
    } catch (error) {
      set(profileId, { state: 'failed', message: message(error) });
    }
  };
  const closeDatabase = async (profileId: string, database: string) => {
    const status = statuses[profileId];
    const opened =
      status?.state === 'connected'
        ? status.databases.find((item) => item.name === database)
        : undefined;
    setStatuses((now) => removeDatabase(now, profileId, database));
    if (opened?.state !== 'open' && opened?.state !== 'reconnecting') return;
    try {
      await closeSession(opened.sessionId);
    } catch {
      // The database is already gone from the UI; a failed goodbye changes nothing.
    }
  };
  return { statusOf, connect, disconnect, openDatabase, closeDatabase };
}

/** Open one database session and record its progress. */
async function openIn(
  update: Update,
  profileId: string,
  name: string,
): Promise<void> {
  update((now) => setDatabase(now, profileId, { name, state: 'opening' }));
  try {
    const opened = await openSession(profileId, name);
    update((now) =>
      setDatabase(now, profileId, {
        name,
        state: 'open',
        sessionId: opened.sessionId,
      }),
    );
  } catch (error) {
    update((now) =>
      setDatabase(now, profileId, {
        name,
        state: 'failed',
        message: message(error),
      }),
    );
  }
}

function useSessionEvents(update: Update): void {
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    onSessionState((event) => {
      update((now) => applySessionEvent(now, event));
    }).then(
      (unlisten) => {
        if (active) stop = unlisten;
        else unlisten();
      },
      () => {
        // Without the native shell there are no session events; commands report their own errors.
      },
    );
    return () => {
      active = false;
      stop?.();
    };
  }, [update]);
}
