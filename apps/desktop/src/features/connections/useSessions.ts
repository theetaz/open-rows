// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import {
  closeCatalog,
  closeSession,
  onSessionState,
  openSession,
} from '@openrows/ipc';
import { applySessionEvent, disconnected } from './sessions';
import type { SessionStatus, Statuses } from './sessions';

type Sessions = {
  statusOf: (profileId: string) => SessionStatus;
  connect: (profileId: string) => Promise<void>;
  disconnect: (profileId: string) => Promise<void>;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** One session per saved connection, kept in sync with core session events. */
export function useSessions(): Sessions {
  const [statuses, setStatuses] = useState<Statuses>({});
  const set = useCallback((profileId: string, status: SessionStatus) => {
    setStatuses((current) => ({ ...current, [profileId]: status }));
  }, []);
  useSessionEvents(setStatuses);
  const statusOf = (profileId: string): SessionStatus =>
    statuses[profileId] ?? disconnected;
  const connect = async (profileId: string): Promise<void> => {
    set(profileId, { state: 'connecting' });
    try {
      const opened = await openSession(profileId);
      set(profileId, { state: 'connected', ...opened });
    } catch (error) {
      set(profileId, { state: 'failed', message: message(error) });
    }
  };
  const disconnect = async (profileId: string): Promise<void> => {
    const status = statuses[profileId];
    if (status?.state !== 'connected') {
      set(profileId, disconnected);
      return;
    }
    try {
      await closeSession(status.sessionId);
      await closeCatalog(profileId);
      set(profileId, disconnected);
    } catch (error) {
      set(profileId, { state: 'failed', message: message(error) });
    }
  };
  return { statusOf, connect, disconnect };
}

function useSessionEvents(
  setStatuses: (change: (current: Statuses) => Statuses) => void,
): void {
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    onSessionState((event) => {
      setStatuses((current) => applySessionEvent(current, event));
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
  }, [setStatuses]);
}
