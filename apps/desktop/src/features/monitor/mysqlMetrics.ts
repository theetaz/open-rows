// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
/** Server-scoped MySQL/MariaDB counters, with InnoDB-specific cache semantics. */
export const MYSQL_METRICS: readonly Metric[] = [
  {
    title: 'Active sessions',
    names: ['sessions:active'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Idle sessions',
    names: ['sessions:idle'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Idle in transaction',
    names: ['sessions:idle in transaction'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Waiting sessions',
    names: ['sessions:waiting'],
    unit: 'count',
    scope: 'Server',
  },
  { title: 'Queries/s', names: ['queries'], unit: '/s', scope: 'Server' },
  {
    title: 'Threads connected',
    names: ['threads_connected'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Threads running',
    names: ['threads_running'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'InnoDB buffer pool hit ratio',
    names: ['buffer_pool_hit_ratio'],
    unit: '%',
    scope: 'Server',
  },
  { title: 'Rows read/s', names: ['rows_read'], unit: '/s', scope: 'Server' },
  {
    title: 'Rows written/s',
    names: ['rows_inserted', 'rows_updated', 'rows_deleted'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'Temporary disk tables/s',
    names: ['temp_disk_tables'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'Slow queries/s',
    names: ['slow_queries'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'Aborted connects/s',
    names: ['aborted_connects'],
    unit: '/s',
    scope: 'Server',
  },
  { title: 'Deadlocks/s', names: ['deadlocks'], unit: '/s', scope: 'Server' },
  {
    title: 'Replication applier lag',
    names: ['replay_lag_ms'],
    unit: 'ms',
    scope: 'Server',
  },
];
