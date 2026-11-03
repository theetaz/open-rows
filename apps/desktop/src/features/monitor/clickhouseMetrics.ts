// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
import { t } from './messages/lazyMetrics';
/** Node-scoped ClickHouse event rates and gauges; no cache ratio is invented. */
export const CLICKHOUSE_METRICS: readonly Metric[] = [
  {
    title: t('runningQueries'),
    names: ['running_queries'],
    unit: 'count',
    scope: 'Node',
  },
  { title: t('queriesS'), names: ['queries'], unit: '/s', scope: 'Node' },
  {
    title: t('failedQueriesS'),
    names: ['failed_queries'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: t('selectedRowsS'),
    names: ['selected_rows'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: t('insertedRowsS'),
    names: ['inserted_rows'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: t('delayedRejectedInserts'),
    names: ['delayed_inserts', 'rejected_inserts'],
    unit: '/s',
    scope: 'Node',
  },
  {
    title: t('runningMerges'),
    names: ['running_merges'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: t('maxPartsPer'),
    names: ['max_parts_per_partition'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: t('trackedMemory'),
    names: ['memory_tracked_bytes'],
    unit: 'bytes',
    scope: 'Node',
  },
  {
    title: t('connections'),
    names: ['tcp_connections', 'http_connections'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: t('readOnlyReplicas'),
    names: ['readonly_replicas'],
    unit: 'count',
    scope: 'Node',
  },
  {
    title: t('replicationDelay'),
    names: ['replay_lag_ms'],
    unit: 'ms',
    scope: 'Node',
  },
];
/** ClickHouse panels: no blocking chains or index usage statistics exist. */
export const CLICKHOUSE_LABELS: Readonly<Record<string, string>> = {
  activity: t('runningQueries'),
  statements: t('queryLog'),
  tables: t('parts'),
  io: t('merges'),
};
export const CLICKHOUSE_HIDDEN: ReadonlySet<string> = new Set([
  'blocking',
  'indexes',
]);
