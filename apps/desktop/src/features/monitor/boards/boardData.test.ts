// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import type { BoardResult } from '@openrows/ipc';
import {
  chartData,
  downsample,
  numericCell,
  statValue,
  tableData,
} from './boardData';
import type { SeriesPoint } from './boardData';

/** Fixture as collected natively: typed columns, text cells, NULL gaps. */
const fixture: BoardResult = {
  columns: [
    { name: 'day', kind: 'time' },
    { name: 'orders', kind: 'number' },
    { name: 'revenue', kind: 'number' },
    { name: 'region', kind: 'text' },
  ],
  rows: [
    ['2026-10-01T00:00:00', '12', '1200.50', 'eu'],
    ['2026-10-02T00:00:00', null, '980.25', 'eu'],
    ['2026-10-03T00:00:00', '-3', '0', 'us'],
    ['2026-10-04T00:00:00', '1e3', '42.125', 'us'],
  ],
  truncated: null,
};

describe('board chart data', () => {
  it('draws exactly the values its table view shows', () => {
    const chart = chartData(fixture, {
      mark: 'line',
      x: 'day',
      y: ['orders', 'revenue'],
    });
    expect(chart.kind).toBe('chart');
    if (chart.kind !== 'chart') return;
    const table = tableData(fixture);
    expect(chart.xKind).toBe('time');
    for (const series of chart.series) {
      const column = table.columns.indexOf(series.name);
      expect(series.points.map((point) => point.y)).toEqual(
        table.rows.map((row) =>
          numericCell(row[column] === 'NULL' ? null : row[column]),
        ),
      );
      expect(series.points.map((point) => point.label)).toEqual(
        table.rows.map((row) => row[0]),
      );
    }
    expect(chart.series[0]?.points.map((point) => point.y)).toEqual([
      12,
      null,
      -3,
      1000,
    ]);
    expect(table.rows[1]?.[1]).toBe('NULL');
  });

  it('falls back to the table for missing or textual columns', () => {
    expect(
      chartData(fixture, { mark: 'bar', x: 'day', y: ['region'] }).kind,
    ).toBe('unsuitable');
    expect(
      chartData(fixture, { mark: 'bar', x: 'nope', y: ['orders'] }).kind,
    ).toBe('unsuitable');
    const category = chartData(fixture, {
      mark: 'bar',
      x: 'region',
      y: ['orders'],
    });
    expect(category.kind === 'chart' && category.xKind).toBe('category');
  });

  it('shows the first cell as a stat', () => {
    expect(statValue(fixture)).toBe('2026-10-01T00:00:00');
    expect(statValue({ ...fixture, rows: [] })).toBe('No rows');
  });

  it('downsampling keeps extrema and gaps', () => {
    const points: SeriesPoint[] = Array.from({ length: 1_000 }, (_, x) => ({
      x,
      label: String(x),
      y: x === 500 ? null : x === 250 ? 9_999 : x % 7,
    }));
    const kept = downsample(points, 50);
    expect(kept.length).toBeLessThanOrEqual(150);
    expect(kept.some((point) => point.y === 9_999)).toBe(true);
    expect(kept.some((point) => point.y === null)).toBe(true);
  });
});
