// SPDX-License-Identifier: Apache-2.0
import { stateLabel } from '../SourceState';
import { timeText } from './model';
import type { ChartPoint } from './model';

/** Most missing-interval lines listed before they are summarised. */
const MAX_INTERVALS = 20;

function missingIntervals(
  points: readonly ChartPoint[],
  range: { fromMs: number; toMs: number },
  cadence: number,
): string[] {
  const limit = cadence * 1.5;
  const times = [range.fromMs, ...points.map((point) => point.x), range.toMs];
  const missing: string[] = [];
  for (let index = 1; index < times.length; index++) {
    const start = times[index - 1] ?? 0;
    const end = times[index] ?? 0;
    if (end - start > limit)
      missing.push(
        `No observation from ${timeText(start)} to ${timeText(end)}`,
      );
  }
  if (missing.length <= MAX_INTERVALS) return missing;
  return [
    ...missing.slice(0, MAX_INTERVALS),
    `${String(missing.length - MAX_INTERVALS)} more missing intervals`,
  ];
}

/**
 * Every unknown/reset/denied observation (counted by state) and every
 * collection interval without an observation; nothing is filled in.
 */
export function metricGaps(
  points: readonly ChartPoint[],
  range: { fromMs: number; toMs: number },
  cadence: number,
): string[] {
  if (!points.length) return ['No observations in this range'];
  const states = new Map<string, number>();
  for (const point of points)
    if (point.y === null)
      states.set(point.state, (states.get(point.state) ?? 0) + 1);
  const counted = [...states].map(
    ([state, count]) =>
      `${String(count)} observation${count === 1 ? '' : 's'}: ${stateLabel(state)}`,
  );
  return [...counted, ...missingIntervals(points, range, cadence)];
}
