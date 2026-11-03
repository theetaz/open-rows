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
import { t } from './messages/lazyDashboard';

type EngineOverview = {
  metrics: readonly Metric[];
  note: string;
  /** ADR-0011 own-session boundary, shown for engines without a process list. */
  boundary?: string;
};
const POSTGRES: EngineOverview = {
  metrics: METRICS,
  note: t('rowsReadReturned'),
};
const MYSQL: EngineOverview = {
  metrics: MYSQL_METRICS,
  note: t('serverStatusIncludes'),
};
const EMBEDDED: EngineOverview = {
  metrics: SQLITE_METRICS,
  note: t('requestCountersCover'),
  boundary: t('ownOpenrowsSessions'),
};
const OVERVIEWS: Record<EngineDto, EngineOverview> = {
  postgres: POSTGRES,
  mysql: MYSQL,
  mariadb: MYSQL,
  mongodb: {
    metrics: [],
    note: t('mongodbMonitoringNot'),
  },
  sqlserver: {
    metrics: SQLSERVER_METRICS,
    note: t('performanceCountersCover'),
  },
  clickhouse: {
    metrics: CLICKHOUSE_METRICS,
    note: t('nodeCountersSystem'),
  },
  sqlite: EMBEDDED,
  'libsql-file': EMBEDDED,
  libsql: {
    metrics: LIBSQL_METRICS,
    note: t('requestCountersRound'),
    boundary: t('ownOpenrowsRequests'),
  },
  duckdb: {
    metrics: DUCKDB_METRICS,
    note: t('updatedAfterQuery'),
    boundary: t('ownOpenrowsSession'),
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
          ? t('storedAggregateHistory', { note: overview.note })
          : t('liveTenMinute', { note: overview.note })}
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
        {latest
          ? t('scopeScopeState', {
              scope: metric.scope,
              state: stateLabel(latest.state),
              value: new Date(latest.collectedMs).toLocaleTimeString(),
            })
          : t('scopeScopeState2', {
              scope: metric.scope,
              state: stateLabel('unknown'),
            })}
      </p>
      {window.previous.length > 0 && (
        <p className={styles.note}>
          {t('previousWindowPrevious', {
            previous: metricValue(previous, metric.unit),
          })}
        </p>
      )}
      <MetricChart title={metric.title} points={points} range={range} />
    </article>
  );
}
