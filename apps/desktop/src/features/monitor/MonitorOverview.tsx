// SPDX-License-Identifier: Apache-2.0
import { useMemo } from 'react';
import type { MonitorData, Point } from './monitorData';
import { METRICS, metricPoints, metricValue } from './metrics';
import type { Metric } from './metrics';
import { MetricChart } from './MetricChart';
import { SourceState, stateLabel } from './SourceState';
import styles from './Monitor.module.css';

/** Fixed dashboard metrics with source freshness, gap charts and prior-window comparisons. */
export function MonitorOverview(props: {
  data: MonitorData | null;
  points: readonly Point[];
  previous: readonly Point[];
  now: number;
  duration: number;
  end: number;
  history: boolean;
}): React.JSX.Element {
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
      <p className={styles.note}>
        {props.history
          ? 'Stored aggregate history; each bucket preserves a missing/reset state. Missing collection intervals are gaps. Other panels show live source tables.'
          : 'Live ten-minute window. Database counters need two samples; no baseline or a reset means unknown.'}{' '}
        Rows read = returned + fetched; rows written = inserted + updated +
        deleted. Cache ratio uses interval block hits/reads.
      </p>
      <div className={styles.tiles}>
        {METRICS.map((metric) => (
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
  const cadence = window.history
    ? Math.floor(Math.min(86_400_000, window.duration * 2) / 300) + 1
    : metric.scope === 'Server' && metric.unit === 'ms'
      ? 60_000
      : metric.scope === 'Database'
        ? Math.max(
            15_000,
            window.data?.reply.sources.find(
              (source) => source.source === 'database',
            )?.intervalMs ?? 15_000,
          )
        : (window.data?.reply.sources.find(
            (source) => source.source === 'activity',
          )?.intervalMs ?? 5000);
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
