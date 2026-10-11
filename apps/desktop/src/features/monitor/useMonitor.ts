// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import {
  monitorSnapshot,
  onMonitorChanged,
  pauseMonitor,
  startMonitor,
  stopMonitor,
} from '@openrows/ipc';
import type { MonitorTarget } from '@openrows/ipc';
import { readMonitorData } from './monitorData';
import type { MonitorData } from './monitorData';
import { MonitorDecoder } from './monitorDecoder';

/** Live collector UI state; only core owns polling and sampling. */
export type MonitorControls = {
  data: MonitorData | null;
  error: string | null;
  paused: boolean;
  setPaused: (paused: boolean) => void;
  interval: number;
  setInterval: (interval: number) => void;
  now: number;
};
const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Explicit lifecycle, serialized admission and late-reply rejection for one visible view. */
export function useMonitor(
  target: MonitorTarget,
  enabled: boolean,
): MonitorControls {
  const [data, setData] = useState<MonitorData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const [interval, setInterval] = useState(5000);
  const [now, setNow] = useState(() => Date.now());
  const queue = useRef(Promise.resolve());
  useEffect(() => {
    if (!enabled) return undefined;
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [enabled]);
  useEffect(() => {
    const scope = { profileId: target.profileId, database: target.database };
    const lifecycleState = { live: true };
    const isLive = (): boolean => lifecycleState.live;
    const lifecycle = async (): Promise<void> => {
      if (!enabled || paused || !isLive()) return;
      const stop = await collect({
        scope,
        interval,
        live: () => lifecycleState.live,
        onError: (failure) => {
          if (lifecycleState.live) setError(message(failure));
        },
        onData: setData,
        onDispose: (stop) => {
          unlisten = stop;
          if (!isLive()) stop();
        },
      });
      if (isLive()) unlisten = stop;
      else stop();
    };
    let unlisten: (() => void) | undefined;
    queue.current = queue.current
      .then(lifecycle)
      .catch(async (failure: unknown) => {
        await stopMonitor(scope).catch((stopFailure: unknown) => {
          if (isLive()) setError(message(stopFailure));
        });
        if (isLive()) setError(message(failure));
      });
    return () => {
      lifecycleState.live = false;
      unlisten?.();
      // Cancel pending native discovery immediately, then serialize cleanup before a new start.
      void stopMonitor(scope).catch((failure: unknown) => {
        setError(message(failure));
      });
      queue.current = queue.current
        .then(() => (paused ? pauseMonitor(scope) : stopMonitor(scope)))
        .catch((failure: unknown) => {
          setError(message(failure));
        });
    };
  }, [target.profileId, target.database, enabled, paused, interval]);
  return {
    data: enabled ? data : null,
    error,
    paused,
    setPaused,
    interval,
    setInterval,
    now,
  };
}

type CollectOptions = {
  scope: MonitorTarget;
  interval: number;
  live: () => boolean;
  onData: (data: MonitorData) => void;
  onError: (error: unknown) => void;
  onDispose: (stop: () => void) => void;
};
async function collect(options: CollectOptions): Promise<() => void> {
  const { scope, live } = options;
  let previous: MonitorData | null = null;
  let busy = false;
  let pending = false;
  const subscription = { active: true };
  const reply = await startMonitor({
    target: scope,
    intervalMs: options.interval,
  });
  if (!live()) return () => undefined;
  const decoder = new MonitorDecoder();
  let unlisten: (() => void) | undefined;
  const stop = (): void => {
    subscription.active = false;
    unlisten?.();
    decoder.stop();
  };
  options.onDispose(stop);
  const isActive = (): boolean => subscription.active;
  const refresh = async (): Promise<void> => {
    if (!live() || !isActive()) return;
    if (busy) {
      pending = true;
      return;
    }
    busy = true;
    try {
      const snapshot = await monitorSnapshot(scope);
      if (snapshot.generation !== reply.generation) return;
      const next = await readMonitorData(scope, snapshot, previous, (bytes) =>
        decoder.decode(bytes),
      );
      if (live() && isActive()) {
        previous = next;
        options.onData(next);
      }
    } finally {
      busy = false;
      if (pending && live() && isActive()) {
        pending = false;
        void refresh().catch(options.onError);
      }
    }
  };
  try {
    unlisten = await onMonitorChanged((event) => {
      if (
        event.target.profileId.toLowerCase() ===
          scope.profileId.toLowerCase() &&
        event.target.database === scope.database &&
        event.generation === reply.generation
      )
        void refresh().catch(options.onError);
    });
    if (!live()) unlisten();
    await refresh();
  } catch (error) {
    stop();
    throw error;
  }
  return stop;
}
