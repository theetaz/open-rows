// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
import { t } from './messages/lazyMetrics';
/** Server-scoped MySQL/MariaDB counters, with InnoDB-specific cache semantics. */
export const MYSQL_METRICS: readonly Metric[] = [
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
    names: ['sessions:idle in transaction'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('waitingSessions'),
    names: ['sessions:waiting'],
    unit: 'count',
    scope: 'Server',
  },
  { title: t('queriesS'), names: ['queries'], unit: '/s', scope: 'Server' },
  {
    title: t('threadsConnected'),
    names: ['threads_connected'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('threadsRunning'),
    names: ['threads_running'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('innodbBufferPool'),
    names: ['buffer_pool_hit_ratio'],
    unit: '%',
    scope: 'Server',
  },
  { title: t('rowsReadS'), names: ['rows_read'], unit: '/s', scope: 'Server' },
  {
    title: t('rowsWrittenS'),
    names: ['rows_inserted', 'rows_updated', 'rows_deleted'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('temporaryDiskTables'),
    names: ['temp_disk_tables'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('slowQueriesS'),
    names: ['slow_queries'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('abortedConnectsS'),
    names: ['aborted_connects'],
    unit: '/s',
    scope: 'Server',
  },
  { title: t('deadlocksS'), names: ['deadlocks'], unit: '/s', scope: 'Server' },
  {
    title: t('replicationApplierLag'),
    names: ['replay_lag_ms'],
    unit: 'ms',
    scope: 'Server',
  },
];
