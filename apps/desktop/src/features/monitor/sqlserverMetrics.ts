// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
/** Instance-wide SQL Server performance counters and request states. */
export const SQLSERVER_METRICS: readonly Metric[] = [
  {
    title: 'Active requests',
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
    title: 'Waiting requests',
    names: ['sessions:waiting'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Batch requests/s',
    names: ['batch_requests'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'Compilations/s',
    names: ['compilations'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'Re-compilations/s',
    names: ['recompilations'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: 'User connections',
    names: ['user_connections'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: 'Blocked processes',
    names: ['processes_blocked'],
    unit: 'count',
    scope: 'Server',
  },
  { title: 'Lock waits/s', names: ['lock_waits'], unit: '/s', scope: 'Server' },
  { title: 'Deadlocks/s', names: ['deadlocks'], unit: '/s', scope: 'Server' },
  {
    title: 'CPU usage (default pool)',
    names: ['cpu_usage'],
    unit: '%',
    scope: 'Server',
  },
  {
    title: 'Buffer cache hit ratio',
    names: ['buffer_cache_hit_ratio'],
    unit: '%',
    scope: 'Server',
  },
  {
    title: 'Page life expectancy',
    names: ['page_life_expectancy'],
    unit: 's',
    scope: 'Server',
  },
  {
    title: 'Server memory',
    names: ['total_server_memory_kb'],
    unit: 'KB',
    scope: 'Server',
  },
  {
    title: 'Target server memory',
    names: ['target_server_memory_kb'],
    unit: 'KB',
    scope: 'Server',
  },
  {
    title: 'Memory grants pending',
    names: ['memory_grants_pending'],
    unit: 'count',
    scope: 'Server',
  },
];
