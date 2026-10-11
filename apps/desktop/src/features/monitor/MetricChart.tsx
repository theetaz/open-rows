// SPDX-License-Identifier: Apache-2.0
import { memo, useState } from 'react';
import type { Point } from './monitorData';
import { stateLabel } from './SourceState';
import styles from './Monitor.module.css';

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

/** Accessible bounded chart with lazily mounted keyboard-readable observations. */
export const MetricChart = memo(function MetricChart(props: {
  title: string;
  points: readonly Point[];
  range: { start: number; end: number; cadence: number };
}): React.JSX.Element {
  const [table, setTable] = useState(false);
  const points = props.points.slice(-300);
  return (
    <>
      <svg
        className={styles.chart}
        viewBox="0 0 300 72"
        role="img"
        aria-label={`${props.title} trend; missing observations are gaps`}
      >
        <title>
          {props.title} — {new Date(props.range.start).toLocaleTimeString()} to{' '}
          {new Date(props.range.end).toLocaleTimeString()}; unknown/reset
          observations are gaps
        </title>
        <path d={chartPath(points, props.range)} />
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
      <details
        onToggle={(event) => {
          setTable(event.currentTarget.open);
        }}
      >
        <summary>Data table for {props.title}</summary>
        {table && (
          <table className={styles.table}>
            <caption>{props.title} observations</caption>
            <thead>
              <tr>
                <th scope="col">Collected</th>
                <th scope="col">Value</th>
                <th scope="col">State</th>
              </tr>
            </thead>
            <tbody>
              {points.map((point) => (
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
    </>
  );
});
