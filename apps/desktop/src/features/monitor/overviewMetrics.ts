// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { CLICKHOUSE_METRICS } from './clickhouseMetrics';
import { LIBSQL_METRICS, SQLITE_METRICS } from './embeddedMetrics';
import { METRICS } from './metrics';
import type { Metric } from './metrics';
import type { MonitorData } from './monitorData';
import { MYSQL_METRICS } from './mysqlMetrics';
import { SQLSERVER_METRICS } from './sqlserverMetrics';

/** The fixed overview series of an engine. */
export function overviewMetrics(
  engine: EngineDto | undefined,
): readonly Metric[] {
  switch (engine) {
    case 'sqlite':
    case 'libsql-file':
      return SQLITE_METRICS;
    case 'libsql':
      return LIBSQL_METRICS;
    case 'sqlserver':
      return SQLSERVER_METRICS;
    case 'clickhouse':
      return CLICKHOUSE_METRICS;
    case 'mysql':
    case 'mariadb':
      return MYSQL_METRICS;
    default:
      return METRICS;
  }
}

/** Collector source a metric is sampled from. */
export function metricSource(metric: Metric): 'activity' | 'database' {
  const name = metric.names[0] ?? '';
  return name.startsWith('sessions:') || name.startsWith('waits:')
    ? 'activity'
    : 'database';
}

/** Expected spacing of a metric's observations; longer steps are gaps. */
export function metricCadence(
  window: { data: MonitorData | null; duration: number; history: boolean },
  metric: Metric,
): number {
  if (window.history)
    return Math.floor(Math.min(86_400_000, window.duration * 2) / 300) + 1;
  if (metric.unit === 'ms' && metric.scope !== 'Database') return 60_000;
  const source = metricSource(metric);
  const floor = source === 'activity' ? 5000 : 15_000;
  return Math.max(
    floor,
    window.data?.reply.sources.find((item) => item.source === source)
      ?.intervalMs ?? floor,
  );
}
