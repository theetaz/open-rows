// SPDX-License-Identifier: Apache-2.0
import type { Metric } from './metrics';
import { t } from './messages/lazyMetrics';
/** Instance-wide SQL Server performance counters and request states. */
export const SQLSERVER_METRICS: readonly Metric[] = [
  {
    title: t('activeRequests'),
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
    title: t('waitingRequests'),
    names: ['sessions:waiting'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('batchRequestsS'),
    names: ['batch_requests'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('compilationsS'),
    names: ['compilations'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('reCompilationsS'),
    names: ['recompilations'],
    unit: '/s',
    scope: 'Server',
  },
  {
    title: t('userConnections'),
    names: ['user_connections'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('blockedProcesses'),
    names: ['processes_blocked'],
    unit: 'count',
    scope: 'Server',
  },
  {
    title: t('lockWaitsS'),
    names: ['lock_waits'],
    unit: '/s',
    scope: 'Server',
  },
  { title: t('deadlocksS'), names: ['deadlocks'], unit: '/s', scope: 'Server' },
  {
    title: t('cpuUsageDefault'),
    names: ['cpu_usage'],
    unit: '%',
    scope: 'Server',
  },
  {
    title: t('bufferCacheHit'),
    names: ['buffer_cache_hit_ratio'],
    unit: '%',
    scope: 'Server',
  },
  {
    title: t('pageLifeExpectancy'),
    names: ['page_life_expectancy'],
    unit: 's',
    scope: 'Server',
  },
  {
    title: t('serverMemory'),
    names: ['total_server_memory_kb'],
    unit: 'KB',
    scope: 'Server',
  },
  {
    title: t('targetServerMemory'),
    names: ['target_server_memory_kb'],
    unit: 'KB',
    scope: 'Server',
  },
  {
    title: t('memoryGrantsPending'),
    names: ['memory_grants_pending'],
    unit: 'count',
    scope: 'Server',
  },
];
