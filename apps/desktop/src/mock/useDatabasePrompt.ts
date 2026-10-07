// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { Connections } from '../features/connections';

/**
 * Ask for a database (the ⌘K panel) once per connect when the active
 * connection has no configured database and none is open yet.
 */
export function useDatabasePrompt(
  connections: Connections,
  open: (mode: 'database') => void,
): void {
  const profile = connections.active;
  const status = profile ? connections.statusOf(profile.id) : undefined;
  const key =
    profile &&
    status?.state === 'connected' &&
    !profile.database &&
    !status.databases.length
      ? `${profile.id}\n${String(status.generation)}`
      : null;
  const prompted = useRef(new Set<string>());
  useEffect(() => {
    if (!key || prompted.current.has(key)) return;
    prompted.current.add(key);
    open('database');
  }, [key, open]);
}
