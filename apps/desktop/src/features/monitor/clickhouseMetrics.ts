// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
/** Node-scoped ClickHouse event rates and gauges; no cache ratio is invented. */
export const CLICKHOUSE_METRICS: readonly Metric[] = [
  {
    title: 'Running queries',
    names: ['running_queries'],
    unit: 'count',
    scope: 'Node',
  },
  { title: 'Queries/s', names: ['queries'], unit: '/s', scope: 'Node' },
  {
    title: 'Failed queries/s',
    names: ['failed_queries'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: 'Selected rows/s',
    names: ['selected_rows'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: 'Inserted rows/s',
    names: ['inserted_rows'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: 'Delayed or rejected inserts/s',
    names: ['delayed_inserts', 'rejected_inserts'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: 'Running merges',
    names: ['running_merges'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: 'Max parts per partition',
    names: ['max_parts_per_partition'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: 'Tracked memory',
    names: ['memory_tracked_bytes'],
    unit: 'bytes',
    scope: 'Node',
  },
  {
    title: 'Connections',
    names: ['tcp_connections', 'http_connections'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: 'Read-only replicas',
    names: ['readonly_replicas'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: 'Replication delay',
    names: ['replay_lag_ms'],
    unit: 'ms',
    scope: 'Node',
  },
];
/** ClickHouse panels: no blocking chains or index usage statistics exist. */
export const CLICKHOUSE_LABELS: Readonly<Record<string, string>> = {
  activity: 'Running queries',
  statements: 'Query log',
  tables: 'Parts',
  io: 'Merges',
};
export const CLICKHOUSE_HIDDEN: ReadonlySet<string> = new Set([
  'blocking',
  'indexes',
]);
