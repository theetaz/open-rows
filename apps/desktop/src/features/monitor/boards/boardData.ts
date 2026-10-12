// SPDX-License-Identifier: Apache-2.0
import type { BoardResult, ChartMapping } from '@openrows/ipc';

/** One chart observation; `y === null` is a gap, never a zero. */
export type SeriesPoint = { x: number; label: string; y: number | null };
export type Series = { name: string; points: SeriesPoint[] };
export type XKind = 'number' | 'time' | 'category';
type ChartData =
  | { kind: 'chart'; xKind: XKind; series: Series[] }
  | { kind: 'unsuitable'; reason: string };

/** Numeric value of a result cell; NULL and non-numbers are gaps. */
export function numericCell(cell: string | null | undefined): number | null {
  if (cell === null || cell === undefined || cell.trim() === '') return null;
  const value = Number(cell);
  return Number.isFinite(value) ? value : null;
}

function xValue(kind: XKind, cell: string | null, row: number): number {
  if (kind === 'number') return numericCell(cell) ?? Number.NaN;
  if (kind === 'time') return cell === null ? Number.NaN : Date.parse(cell);
  return row;
}

/** Explicit column mapping of a result onto chart series; table fallback otherwise. */
export function chartData(
  result: BoardResult,
  mapping: ChartMapping,
): ChartData {
  const index = (name: string): number =>
    result.columns.findIndex((column) => column.name === name);
  const x = index(mapping.x);
  if (x < 0)
    return { kind: 'unsuitable', reason: `No column ${mapping.x} for x.` };
  const ys = mapping.y.map(index);
  const missing = mapping.y.find((_, at) => ys[at] === undefined || ys[at] < 0);
  if (missing !== undefined)
    return { kind: 'unsuitable', reason: `No column ${missing} for y.` };
  const textual = ys.find((at) => result.columns[at]?.kind !== 'number');
  if (textual !== undefined)
    return {
      kind: 'unsuitable',
      reason: `Column ${result.columns[textual]?.name ?? ''} is not numeric; showing the table.`,
    };
  const columnKind = result.columns[x]?.kind;
  const xKind: XKind =
    columnKind === 'number'
      ? 'number'
      : columnKind === 'time'
        ? 'time'
        : 'category';
  const series = ys.map((at) => ({
    name: result.columns[at]?.name ?? '',
    points: result.rows.map((row, rowIndex) => ({
      x: xValue(xKind, row[x] ?? null, rowIndex),
      label: row[x] ?? 'NULL',
      y: numericCell(row[at]),
    })),
  }));
  return { kind: 'chart', xKind, series };
}

/** Display text of every cell, as the accessible table shows it. */
export function tableData(result: BoardResult): {
  columns: string[];
  rows: string[][];
} {
  return {
    columns: result.columns.map((column) => column.name),
    rows: result.rows.map((row) => row.map((cell) => cell ?? 'NULL')),
  };
}

/** First value of the first row, for a stat card. */
export function statValue(result: BoardResult): string {
  const cell = result.rows[0]?.[0];
  if (cell === undefined) return 'No rows';
  return cell ?? 'NULL';
}

/**
 * At most `buckets × 2` points: each bucket keeps its minimum and maximum,
 * and a bucket containing a gap keeps it, so gaps and extrema survive.
 */
export function downsample(
  points: readonly SeriesPoint[],
  buckets: number,
): SeriesPoint[] {
  if (points.length <= buckets * 2) return [...points];
  const size = Math.ceil(points.length / buckets);
  const kept: SeriesPoint[] = [];
  for (let start = 0; start < points.length; start += size) {
    const bucket = points.slice(start, start + size);
    const gap = bucket.find((point) => point.y === null);
    const valued = bucket.filter((point) => point.y !== null);
    const low = valued.reduce<SeriesPoint | undefined>(
      (best, point) => (!best || (point.y ?? 0) < (best.y ?? 0) ? point : best),
      undefined,
    );
    const high = valued.reduce<SeriesPoint | undefined>(
      (best, point) => (!best || (point.y ?? 0) > (best.y ?? 0) ? point : best),
      undefined,
    );
    const chosen = [low, high, gap].filter(
      (point): point is SeriesPoint => point !== undefined,
    );
    const unique = [...new Set(chosen)];
    unique.sort((a, b) => bucket.indexOf(a) - bucket.indexOf(b));
    kept.push(...unique);
  }
  return kept;
}
