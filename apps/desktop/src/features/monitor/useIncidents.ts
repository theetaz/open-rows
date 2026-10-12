// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { monitorIncidents, onMonitorIncident } from '@openrows/ipc';
import type { IncidentsReply, MonitorTarget } from '@openrows/ipc';

/** Read incidents on open, on each in-app notification and when the live revision moves. */
export function useIncidents(
  target: MonitorTarget,
  revision: string,
): { reply: IncidentsReply | null; error: string | null } {
  const [reply, setReply] = useState<IncidentsReply | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { profileId, database } = target;
  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;
    const refresh = (): void => {
      monitorIncidents({ profileId, database }).then(
        (value) => {
          if (live) {
            setReply(value);
            setError(null);
          }
        },
        (failure: unknown) => {
          if (live) setError(String(failure));
        },
      );
    };
    refresh();
    void onMonitorIncident((event) => {
      if (
        event.target.profileId.toLowerCase() === profileId.toLowerCase() &&
        event.target.database === database
      )
        refresh();
    }).then((stop) => {
      if (live) unlisten = stop;
      else stop();
    });
    return () => {
      live = false;
      unlisten?.();
    };
  }, [profileId, database, revision]);
  return { reply, error };
}
