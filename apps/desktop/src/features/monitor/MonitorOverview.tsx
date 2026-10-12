// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import type { MonitorData, Point } from './monitorData';
import type { EngineDto } from '@openrows/ipc';
import { MYSQL_METRICS } from './mysqlMetrics';
import { SQLSERVER_METRICS } from './sqlserverMetrics';
import { CLICKHOUSE_METRICS } from './clickhouseMetrics';
import {
  DUCKDB_METRICS,
  LIBSQL_METRICS,
  SQLITE_METRICS,
} from './embeddedMetrics';
import { METRICS, metricPoints, metricValue } from './metrics';
import { metricCadence } from './overviewMetrics';
import type { Metric } from './metrics';
import { MetricChart } from './MetricChart';
import { SourceState, stateLabel } from './SourceState';
import styles from './Monitor.module.css';

type EngineOverview = {
  metrics: readonly Metric[];
  note: string;
  /** ADR-0011 own-session boundary, shown for engines without a process list. */
  boundary?: string;
};
const POSTGRES: EngineOverview = {
  metrics: METRICS,
  note: 'Rows read = returned + fetched; rows written = inserted + updated + deleted. Cache ratio uses interval block hits/reads.',
};
const MYSQL: EngineOverview = {
  metrics: MYSQL_METRICS,
  note: 'Server status includes all databases and collector queries. Rows are InnoDB counters; buffer pool hit ratio is 1 − physical reads/read requests. Missing engine counters remain unknown.',
};
const EMBEDDED: EngineOverview = {
  metrics: SQLITE_METRICS,
  note: 'Request counters cover statements run from OpenRows editors and grids against this file. File gauges come from cheap PRAGMAs, file sizes and the WAL index; integrity and quick checks never run automatically. Missing WAL values mean the file is not in WAL mode.',
  boundary: 'Own OpenRows sessions only — other applications are not visible.',
};
const OVERVIEWS: Record<EngineDto, EngineOverview> = {
  postgres: POSTGRES,
  mysql: MYSQL,
  mariadb: MYSQL,
  sqlserver: {
    metrics: SQLSERVER_METRICS,
    note: 'Performance counters cover the whole instance, including collector queries. CPU is the default resource pool share; buffer cache hit ratio and page life expectancy are current values. Missing counters remain unknown.',
  },
  clickhouse: {
    metrics: CLICKHOUSE_METRICS,
    note: 'Node counters from system events, metrics and asynchronous metrics include collector queries. Events are interval rates; running queries, merges, parts and memory are gauges. Missing or denied counters remain unknown.',
  },
  sqlite: EMBEDDED,
  'libsql-file': EMBEDDED,
  libsql: {
    metrics: LIBSQL_METRICS,
    note: 'Request counters and round trip are measured by OpenRows. Server CPU, memory, storage and other clients’ queries are unavailable over Hrana and are never estimated.',
    boundary:
      'Own OpenRows requests only — server-wide metrics unavailable over Hrana.',
  },
  duckdb: {
    metrics: DUCKDB_METRICS,
    note: 'Updated after query completion. Request counters cover statements run from OpenRows editors and grids. Storage, memory, temp and thread values are read on the owning session after the running statement; a busy session leaves a gap.',
    boundary: 'Own OpenRows session only — other processes are not visible.',
  },
};

/** Fixed dashboard metrics with source freshness, gap charts and prior-window comparisons. */
export function MonitorOverview(props: {
  engine?: EngineDto;
  data: MonitorData | null;
  points: readonly Point[];
  previous: readonly Point[];
  now: number;
  duration: number;
  end: number;
  history: boolean;
}): React.JSX.Element {
  const overview = props.engine ? OVERVIEWS[props.engine] : POSTGRES;
  return (
    <>
      {['activity', 'database', 'replication'].map((source) => (
        <SourceState
          key={source}
          descriptor={props.data?.reply.capabilities.find(
            (item) => item.source === source,
          )}
          status={props.data?.reply.sources.find(
            (item) => item.source === source,
          )}
          now={props.now}
        />
      ))}
      {overview.boundary && (
        <p role="note" className={styles.note}>
          {overview.boundary}
        </p>
      )}
      <p className={styles.note}>
        {props.history
          ? 'Stored aggregate history; each bucket preserves a missing/reset state. Missing collection intervals are gaps. Other panels show live source tables.'
          : 'Live ten-minute window. Database counters need two samples; no baseline or a reset means unknown.'}{' '}
        {overview.note}
      </p>
      <div className={styles.tiles}>
        {overview.metrics.map((metric) => (
          <MetricTile key={metric.title} metric={metric} window={props} />
        ))}
      </div>
    </>
  );
}

function MetricTile({
  metric,
  window,
}: {
  metric: Metric;
  window: Parameters<typeof MonitorOverview>[0];
}): React.JSX.Element {
  const points = useMemo(
    () => metricPoints(window.points, metric),
    [window.points, metric],
  );
  const previous = metricPoints(window.previous, metric).at(-1);
  const latest = points.at(-1);
  const cadence = metricCadence(window, metric);
  const range = useMemo(
    () => ({ start: window.end - window.duration, end: window.end, cadence }),
    [window.end, window.duration, cadence],
  );
  return (
    <article
      key={metric.title}
      className={styles.tile}
      aria-label={metric.title}
    >
      <h3>{metric.title}</h3>
      <p className={styles.value}>{metricValue(latest, metric.unit)}</p>
      <p className={styles.note}>
        {metric.scope} scope · {stateLabel(latest?.state ?? 'unknown')}
        {latest
          ? ` · ${new Date(latest.collectedMs).toLocaleTimeString()}`
          : ''}
      </p>
      {window.previous.length > 0 && (
        <p className={styles.note}>
          Previous window: {metricValue(previous, metric.unit)}
        </p>
      )}
      <MetricChart title={metric.title} points={points} range={range} />
    </article>
  );
}
