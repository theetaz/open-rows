// SPDX-License-Identifier: Apache-2.0
import { memo } from 'react';
import type { Mark } from '@openrows/ipc';
import { downsample } from './boardData';
import type { Series, SeriesPoint, XKind } from './boardData';
import styles from './Boards.module.css';

const WIDTH = 300;
const HEIGHT = 120;
const PAD = 4;

type Scale = {
  x: (point: SeriesPoint) => number;
  y: (value: number) => number;
};

function extent(values: number[]): [number, number] {
  const finite = values.filter(Number.isFinite);
  if (!finite.length) return [0, 1];
  const low = Math.min(...finite);
  const high = Math.max(...finite);
  return low === high ? [low, low + 1] : [low, high];
}

function scale(series: Series[], mark: Mark): Scale {
  const all = series.flatMap((s) => s.points);
  const [xLow, xHigh] = extent(all.map((point) => point.x));
  const ys = all.map((point) => point.y ?? Number.NaN);
  const [yLow, yHigh] = extent(
    mark === 'bar' || mark === 'area' ? [0, ...ys] : ys,
  );
  const span = WIDTH - PAD * 2;
  return {
    x: (point) => PAD + ((point.x - xLow) / (xHigh - xLow || 1)) * span,
    y: (value) =>
      HEIGHT - PAD - ((value - yLow) / (yHigh - yLow)) * (HEIGHT - PAD * 2),
  };
}

/** Line path that breaks at gaps; `area` closes each segment to the baseline. */
function path(points: SeriesPoint[], at: Scale, area: boolean): string {
  let text = '';
  let segment: string[] = [];
  let first = 0;
  let last = 0;
  const close = (): void => {
    if (!segment.length) return;
    text += `M${segment.join('L')}`;
    if (area)
      text += `L${last.toFixed(1)},${at.y(0).toFixed(1)}L${first.toFixed(1)},${at.y(0).toFixed(1)}Z`;
    text += ' ';
    segment = [];
  };
  for (const point of points) {
    if (point.y === null || !Number.isFinite(point.x)) {
      close();
      continue;
    }
    const x = at.x(point);
    if (!segment.length) first = x;
    last = x;
    segment.push(`${x.toFixed(1)},${at.y(point.y).toFixed(1)}`);
  }
  close();
  return text;
}

function SeriesMarks(props: {
  series: Series;
  index: number;
  count: number;
  mark: Mark;
  at: Scale;
}): React.JSX.Element {
  const { series, index, mark, at } = props;
  const className = styles[`series${String(index % 6)}`];
  if (mark === 'line' || mark === 'area')
    return (
      <path
        className={className}
        data-mark={mark}
        d={path(downsample(series.points, 300), at, mark === 'area')}
      />
    );
  const valued = series.points.filter(
    (point) => point.y !== null && Number.isFinite(point.x),
  );
  if (mark === 'scatter')
    return (
      <g className={className} data-mark="scatter">
        {valued.slice(0, 2_000).map((point, offset) => (
          <circle
            key={offset}
            cx={at.x(point)}
            cy={at.y(point.y ?? 0)}
            r="1.8"
          />
        ))}
      </g>
    );
  const width = Math.max(
    1,
    (WIDTH - PAD * 2) / Math.max(1, series.points.length) / props.count - 1,
  );
  return (
    <g className={className} data-mark="bar">
      {valued.slice(0, 2_000).map((point, offset) => {
        const top = at.y(Math.max(0, point.y ?? 0));
        const bottom = at.y(Math.min(0, point.y ?? 0));
        return (
          <rect
            key={offset}
            x={at.x(point) - (width * props.count) / 2 + width * index}
            y={top}
            width={width}
            height={Math.max(0.5, bottom - top)}
          />
        );
      })}
    </g>
  );
}

/** Bounded SVG chart; the widget's table toggle is its accessible data alternative. */
export const BoardChart = memo(function BoardChart(props: {
  title: string;
  mark: Mark;
  xKind: XKind;
  series: Series[];
}): React.JSX.Element {
  const at = scale(props.series, props.mark);
  const rows = props.series[0]?.points.length ?? 0;
  return (
    <figure className={styles.figure}>
      <svg
        className={styles.chart}
        viewBox={`0 0 ${String(WIDTH)} ${String(HEIGHT)}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`${props.title}: ${props.mark} chart of ${props.series
          .map((s) => s.name)
          .join(
            ', ',
          )} over ${String(rows)} ${props.xKind} values; NULL values are gaps`}
      >
        {props.series.map((series, index) => (
          <SeriesMarks
            key={series.name}
            series={series}
            index={index}
            count={props.series.length}
            mark={props.mark}
            at={at}
          />
        ))}
      </svg>
      {props.series.length > 1 && (
        <figcaption className={styles.legend}>
          {props.series.map((series, index) => (
            <span
              key={series.name}
              className={styles[`series${String(index % 6)}`]}
            >
              ■ {series.name}
            </span>
          ))}
        </figcaption>
      )}
    </figure>
  );
});
