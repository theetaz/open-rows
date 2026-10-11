// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { monitorHistory } from '@openrows/ipc';
import type { MonitorTarget } from '@openrows/ipc';
import { monitorSettings } from '../settings/monitorSettings';
import type { Point } from './monitorData';
/** Selected local range; history controls are never available without an opt-in. */
export function useMonitorHistory(
  target: MonitorTarget,
  readable = true,
): {
  enabled: boolean;
  hours: number;
  setHours: (hours: number) => void;
  points: Point[];
  previous: Point[];
  end: number;
  error: string | null;
  refresh: () => void;
} {
  const settings = monitorSettings.useValue();
  const enabled = settings.enabledProfiles.includes(
    target.profileId.toLowerCase(),
  );
  const [hours, setHours] = useState(0);
  const [revision, setRevision] = useState(0);
  const [result, setResult] = useState(() => ({
    points: [] as Point[],
    previous: [] as Point[],
    end: Date.now(),
  }));
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    if (!readable || !enabled || !hours) return undefined;
    const end = Date.now();
    monitorHistory(
      { profileId: target.profileId, database: target.database },
      Math.min(24, hours * 2),
    ).then(
      (observations) => {
        const points: Point[] = observations.flatMap((point) =>
          point.collectedMs === null
            ? []
            : [{ ...point, collectedMs: point.collectedMs }],
        );
        if (!live) return;
        const start = end - hours * 3_600_000;
        setResult({
          points: points.filter((point) => point.collectedMs >= start),
          previous: points.filter((point) => point.collectedMs < start),
          end,
        });
        setError(null);
      },
      (failure: unknown) => {
        if (live)
          setError(
            failure instanceof Error ? failure.message : String(failure),
          );
      },
    );
    return () => {
      live = false;
    };
  }, [readable, enabled, hours, target.profileId, target.database, revision]);
  return {
    enabled,
    hours: enabled ? hours : 0,
    setHours,
    points: enabled && readable ? result.points : [],
    previous: enabled && readable ? result.previous : [],
    end: result.end,
    error,
    refresh: () => {
      setResult({ points: [], previous: [], end: Date.now() });
      setRevision((value) => value + 1);
    },
  };
}
