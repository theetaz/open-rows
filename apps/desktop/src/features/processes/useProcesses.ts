// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import { listProcesses } from '@openrows/ipc';
import type { ProcessDto } from '@openrows/ipc';
import type { RefreshInterval } from './processModel';

/** The listed processes, the last failure and the refresh controls. */
export type Processes = {
  processes: readonly ProcessDto[];
  /** `true` until the first listing answered (or failed). */
  loading: boolean;
  error: string | null;
  interval: RefreshInterval;
  setInterval: (interval: RefreshInterval) => void;
  reload: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * The server's processes for one connection and database, read on its
 * metadata connection; reloads every `interval` seconds while enabled. A
 * listing still in flight is never overtaken: ticks wait for it.
 */
export function useProcesses(
  profileId: string,
  database: string,
  enabled: boolean,
): Processes {
  const [processes, setProcesses] = useState<readonly ProcessDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [interval, setInterval] = useState<RefreshInterval>('5');
  const busy = useRef(false);
  const reload = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    listProcesses(profileId, database)
      .then(
        (listed) => {
          setProcesses(listed);
          setError(null);
        },
        (failure: unknown) => {
          setError(message(failure));
        },
      )
      .finally(() => {
        busy.current = false;
        setLoading(false);
      });
  }, [profileId, database]);
  useEffect(() => {
    if (enabled) reload();
  }, [enabled, reload]);
  useEffect(() => {
    const seconds = Number(interval);
    if (!enabled || !seconds) return undefined;
    const timer = window.setInterval(reload, seconds * 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, interval, reload]);
  return { processes, loading, error, interval, setInterval, reload };
}
