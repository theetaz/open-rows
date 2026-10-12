// SPDX-License-Identifier: Apache-2.0
// The versioned report artifact (FR-ADMIN-08, docs/28 §5, §18). Every export
// format renders this one document, so the same numbers appear in each.

/** Identifies the JSON document; bump `REPORT_VERSION` on breaking changes. */
export const REPORT_SCHEMA = 'openrows.monitor-report';
export const REPORT_VERSION = 1;

/**
 * Where a section's values come from. `gaps` lists every missing, unknown,
 * denied or truncated part explicitly; missing values are never zeros.
 */
export type Evidence = {
  source: string;
  fromMs: number | null;
  toMs: number | null;
  /** Server, Database, Node, File, Own requests or Query result. */
  scope: string;
  gaps: string[];
};

/** A table cell; `null` is unknown or SQL NULL, never zero. */
export type Cell = string | number | null;
export type Column = { name: string; unit: string };
/** One observation; `y === null` is a gap whose reason is `state`. */
export type ChartPoint = {
  x: number;
  label: string;
  y: number | null;
  state: string;
};
export type ChartSeries = { name: string; points: ChartPoint[] };
export type Mark = 'line' | 'area' | 'bar' | 'scatter';
export type XKind = 'time' | 'number' | 'category';

type Base = { id: string; title: string; evidence: Evidence };
export type SummarySection = Base & {
  kind: 'summary';
  items: { label: string; value: string }[];
};
export type ChartSection = Base & {
  kind: 'chart';
  unit: string;
  mark: Mark;
  xKind: XKind;
  /** Expected spacing of time observations; a longer step is a missing interval. */
  cadenceMs: number | null;
  series: ChartSeries[];
};
export type TableSection = Base & {
  kind: 'table';
  columns: Column[];
  rows: Cell[][];
};
export type StatSection = Base & { kind: 'stat'; value: Cell };
export type ReportSection =
  SummarySection | ChartSection | TableSection | StatSection;

export type Report = {
  schema: typeof REPORT_SCHEMA;
  version: typeof REPORT_VERSION;
  title: string;
  generatedMs: number;
  connection: { name: string; engine: string; database: string };
  range: { fromMs: number | null; toMs: number | null; label: string };
  sections: ReportSection[];
};

/** Fixed precision so every format prints the identical number. */
export function roundValue(value: number): number {
  return Number.isInteger(value) ? value : Number(value.toFixed(6));
}

/** Locale-independent text of a number, as JSON prints it. */
export function numberText(value: number): string {
  return String(value);
}

/** Text of a cell; unknown stays explicit. */
export function cellText(cell: Cell, unknown = 'unknown'): string {
  if (cell === null) return unknown;
  return typeof cell === 'number' ? numberText(cell) : cell;
}

/** UTC ISO time, or an explicit unknown. */
export function timeText(ms: number | null): string {
  return ms === null ? 'unknown' : new Date(ms).toISOString();
}

/** Sections that export as CSV rows. */
export function tabular(
  section: ReportSection,
): section is TableSection | ChartSection {
  return section.kind === 'table' || section.kind === 'chart';
}

/** Rows of a table section, or one long-format row per chart observation. */
export function sectionRows(section: TableSection | ChartSection): {
  columns: Column[];
  rows: Cell[][];
} {
  if (section.kind === 'table')
    return { columns: section.columns, rows: section.rows };
  const x = section.xKind === 'time' ? 'collected' : 'x';
  return {
    columns: [
      { name: 'series', unit: '' },
      { name: x, unit: '' },
      { name: 'value', unit: section.unit },
      { name: 'state', unit: '' },
    ],
    rows: section.series.flatMap((series) =>
      series.points.map((point) => [
        series.name,
        section.xKind === 'number' ? point.x : point.label,
        point.y,
        point.state,
      ]),
    ),
  };
}

/** Latest, minimum, maximum and mean of a series' real observations. */
export function seriesStats(series: ChartSeries): {
  latest: ChartPoint | undefined;
  min: number | null;
  max: number | null;
  mean: number | null;
  observed: number;
  gaps: number;
} {
  const values = series.points.flatMap((point) =>
    point.y === null ? [] : [point.y],
  );
  const total = values.reduce((sum, value) => sum + value, 0);
  return {
    latest: series.points.at(-1),
    min: values.length ? Math.min(...values) : null,
    max: values.length ? Math.max(...values) : null,
    mean: values.length ? roundValue(total / values.length) : null,
    observed: values.length,
    gaps: series.points.length - values.length,
  };
}
