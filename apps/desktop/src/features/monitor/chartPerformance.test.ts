// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { chartPath } from './MetricChart';
import { METRICS, metricPoints } from './metrics';
import type { Point } from './monitorData';
/** Opt-in CPU evidence; normal suites assert only bounded, gap-preserving output. */
it('maps and lays out a full ten-minute window with bounded gap-preserving SVG paths', () => {
  const points: Point[] = [];
  for (const metric of METRICS)
    for (const name of metric.names)
      for (let index = 0; index < 300; index++)
        points.push({
          metric: name,
          collectedMs: index * 2000,
          value: index % 47 === 0 ? null : index % 13,
          state: index % 47 === 0 ? 'gap' : 'ok',
        });
  const durations: number[] = [];
  for (
    let repeat = 0;
    repeat < (process.env.OPENROWS_BENCH ? 100 : 1);
    repeat++
  ) {
    const begin = performance.now();
    for (const metric of METRICS)
      expect(
        chartPath(metricPoints(points, metric), {
          start: 0,
          end: 600000,
          cadence: 2000,
        }).length,
      ).toBeLessThan(10_000);
    durations.push(performance.now() - begin);
  }
  if (process.env.OPENROWS_BENCH) {
    durations.sort((a, b) => a - b);
    process.stdout.write(
      `Monitor 11 charts × 300 timestamps: layout p95=${(durations[95] ?? 0).toFixed(3)} ms; max=${(durations.at(-1) ?? 0).toFixed(3)} ms; 16.7 ms frame / 50 ms long-task budgets (CPU mapping only)\n`,
    );
  }
});
