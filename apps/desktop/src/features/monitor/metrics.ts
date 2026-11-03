// SPDX-License-Identifier: Apache-2.0
import type { Point } from './monitorData';
import { t } from './messages/lazyMetrics';
/** Overview metric definition; multiple counters are summed only with complete observations. */
export type Metric = {
  title: string;
  names: readonly string[];
  unit: string;
  scope: string;
};
/** Fixed overview series keep chart mapping bounded and explain server/database scope. */
export const METRICS: readonly Metric[] = [
  {
    title: t('activeSessions'),
    names: ['sessions:active'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('idleSessions'),
    names: ['sessions:idle'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('idleTransaction'),
    names: [
      'sessions:idle in transaction',
      'sessions:idle in transaction (aborted)',
    ],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('waitingSessions'),
    names: ['sessions:waiting'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('transactionsS'),
    names: ['xact_commit', 'xact_rollback'],
    unit: '/s',
    scope: 'Database',
  },
  {
    title: t('cacheHitRatio'),
    names: ['cache_hit_ratio'],
    unit: '%',
    scope: 'Database',
  },
  {
    title: t('temporaryBytesS'),
    names: ['temp_bytes'],
    unit: 'B/s',
    scope: 'Database',
  },
  {
    title: t('deadlocksS'),
    names: ['deadlocks'],
    unit: '/s',
    scope: 'Database',
  },
  {
    title: t('rowsReadS'),
    names: ['tup_returned', 'tup_fetched'],
    unit: '/s',
    scope: 'Database',
  },
  {
    title: t('rowsWrittenS'),
    names: ['tup_inserted', 'tup_updated', 'tup_deleted'],
    unit: '/s',
    scope: 'Database',
  },
  {
    title: t('replicationReplayLag'),
    names: ['replay_lag_ms'],
    unit: 'ms',
    scope: 'Server',
  },
];
/** Map and combine a metric once per window; incomplete/reset samples stay gaps. */
export function metricPoints(
  points: readonly Point[],
  metric: Metric,
): Point[] {
  const buckets = new Map<number, Point[]>();
  for (const point of points) {
    if (
      !metric.names.some(
        (name) => point.metric === name || point.metric.endsWith(`:${name}`),
      )
    )
      continue;
    const bucket = buckets.get(point.collectedMs) ?? [];
    bucket.push(point);
    buckets.set(point.collectedMs, bucket);
  }
  return [...buckets.entries()]
    .sort(([a], [b]) => a - b)
    .map(([collectedMs, samples]) => {
      const invalid = samples.find(
        (point) => point.state !== 'ok' || point.value === null,
      );
      const state =
        invalid?.state ??
        (samples.length === metric.names.length ? 'ok' : 'unknown');
      const value =
        state === 'ok'
          ? samples.reduce((total, point) => total + (point.value ?? 0), 0)
          : null;
      return {
        metric: metric.title,
        collectedMs,
        state,
        value:
          value === null ? null : metric.unit === '%' ? value * 100 : value,
      };
    });
}
/** Format a real observation or its explicit missing state. */
export function metricValue(point: Point | undefined, unit: string): string {
  if (!point || point.state !== 'ok' || point.value === null)
    return t('unknown');
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(point.value)} ${unit}`;
}
