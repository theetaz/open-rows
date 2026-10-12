// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import type { MonitorData, Point } from './monitorData';
import type { EngineDto } from '@openrows/ipc';
import { metricPoints, metricValue } from './metrics';
import { metricCadence, overviewMetrics } from './overviewMetrics';
import type { Metric } from './metrics';
import { MetricChart } from './MetricChart';
import { SourceState, stateLabel } from './SourceState';
import styles from './Monitor.module.css';

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
  const mysql = props.engine === 'mysql' || props.engine === 'mariadb';
  const sqlserver = props.engine === 'sqlserver';
  const clickhouse = props.engine === 'clickhouse';
  const embedded = props.engine === 'sqlite' || props.engine === 'libsql-file';
  const remote = props.engine === 'libsql';
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
      {(embedded || remote) && (
        <p role="note" className={styles.note}>
          {embedded
            ? 'Own OpenRows sessions only — other applications are not visible.'
            : 'Own OpenRows requests only — server-wide metrics unavailable over Hrana.'}
        </p>
      )}
      <p className={styles.note}>
        {props.history
          ? 'Stored aggregate history; each bucket preserves a missing/reset state. Missing collection intervals are gaps. Other panels show live source tables.'
          : 'Live ten-minute window. Database counters need two samples; no baseline or a reset means unknown.'}{' '}
        {embedded
          ? 'Request counters cover statements run from OpenRows editors and grids against this file. File gauges come from cheap PRAGMAs, file sizes and the WAL index; integrity and quick checks never run automatically. Missing WAL values mean the file is not in WAL mode.'
          : remote
            ? 'Request counters and round trip are measured by OpenRows. Server CPU, memory, storage and other clients’ queries are unavailable over Hrana and are never estimated.'
            : sqlserver
              ? 'Performance counters cover the whole instance, including collector queries. CPU is the default resource pool share; buffer cache hit ratio and page life expectancy are current values. Missing counters remain unknown.'
              : clickhouse
                ? 'Node counters from system events, metrics and asynchronous metrics include collector queries. Events are interval rates; running queries, merges, parts and memory are gauges. Missing or denied counters remain unknown.'
                : mysql
                  ? 'Server status includes all databases and collector queries. Rows are InnoDB counters; buffer pool hit ratio is 1 − physical reads/read requests. Missing engine counters remain unknown.'
                  : 'Rows read = returned + fetched; rows written = inserted + updated + deleted. Cache ratio uses interval block hits/reads.'}
      </p>
      <div className={styles.tiles}>
        {overviewMetrics(props.engine).map((metric) => (
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
