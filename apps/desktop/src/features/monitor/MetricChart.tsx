// SPDX-License-Identifier: Apache-2.0
import { memo, useState } from 'react';
import type { Point } from './monitorData';
import { stateLabel } from './SourceState';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

/** Gap-preserving SVG path, at most 300 observations and no animation work. */
export function chartPath(
  points: readonly Point[],
  range: { start: number; end: number; cadence: number },
): string {
  const maximum = Math.max(1, ...points.map((point) => point.value ?? 0));
  let path = '';
  let previous: number | null = null;
  for (const point of points) {
    if (point.value === null || point.state !== 'ok') {
      previous = null;
      continue;
    }
    const x =
      ((point.collectedMs - range.start) /
        Math.max(1, range.end - range.start)) *
      300;
    const y = 64 - (point.value / maximum) * 60;
    const move =
      previous === null || point.collectedMs - previous > range.cadence * 1.5;
    path += `${move ? 'M' : 'L'}${x.toFixed(2)},${y.toFixed(2)} `;
    previous = point.collectedMs;
  }
  return path;
}

/** The newest valid observation, marked in the accent colour. */
function latestMark(
  points: readonly Point[],
  range: { start: number; end: number },
): { x: number; y: number } | null {
  const valid = points.filter((p) => p.state === 'ok' && p.value !== null);
  const last = valid.at(-1);
  if (!last || last.value === null) return null;
  const maximum = Math.max(1, ...points.map((point) => point.value ?? 0));
  return {
    x:
      ((last.collectedMs - range.start) /
        Math.max(1, range.end - range.start)) *
      300,
    y: 64 - (last.value / maximum) * 60,
  };
}

/** Accessible bounded chart with lazily mounted keyboard-readable observations. */
export const MetricChart = memo(function MetricChart(props: {
  title: string;
  points: readonly Point[];
  range: { start: number; end: number; cadence: number };
}): React.JSX.Element {
  const points = props.points.slice(-300);
  const mark = latestMark(points, props.range);
  return (
    <>
      <svg
        className={styles.chart}
        viewBox="0 0 300 72"
        role="img"
        aria-label={t('titleTrendMissing', { title: props.title })}
      >
        <title>
          {t('titleValueValue2', {
            title: props.title,
            value: new Date(props.range.start).toLocaleTimeString(),
            value2: new Date(props.range.end).toLocaleTimeString(),
          })}
        </title>
        <path d={chartPath(points, props.range)} />
        {mark && (
          <circle
            className={styles.chartMark}
            cx={mark.x}
            cy={mark.y}
            r="2.5"
          />
        )}
        {points
          .filter((point) => point.state !== 'ok' || point.value === null)
          .map((point) => (
            <circle
              key={point.collectedMs}
              cx={
                ((point.collectedMs - props.range.start) /
                  Math.max(1, props.range.end - props.range.start)) *
                300
              }
              cy="68"
              r="2"
              fill="currentColor"
            >
              <title>
                {new Date(point.collectedMs).toLocaleTimeString()}:{' '}
                {stateLabel(point.state)}
              </title>
            </circle>
          ))}
      </svg>
      <Observations title={props.title} points={points} />
    </>
  );
});

/** The chart's observations as a table, mounted only while open. */
function Observations(props: {
  title: string;
  points: readonly Point[];
}): React.JSX.Element {
  const [table, setTable] = useState(false);
  return (
    <details
      className={styles.dataTable}
      onToggle={(event) => {
        setTable(event.currentTarget.open);
      }}
    >
      <summary>{t('dataTableTitle', { title: props.title })}</summary>
      {table && (
        <table className={styles.table}>
          <caption>{t('titleObservations', { title: props.title })}</caption>
          <thead>
            <tr>
              <th scope="col">{t('collected')}</th>
              <th scope="col">{t('value')}</th>
              <th scope="col">{t('state')}</th>
            </tr>
          </thead>
          <tbody>
            {props.points.map((point) => (
              <tr key={point.collectedMs}>
                <td>{new Date(point.collectedMs).toLocaleString()}</td>
                <td>
                  {point.state === 'ok' && point.value !== null
                    ? String(point.value)
                    : '—'}
                </td>
                <td>{stateLabel(point.state)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </details>
  );
}
