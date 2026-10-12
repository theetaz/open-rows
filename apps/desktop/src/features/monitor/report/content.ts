// SPDX-License-Identifier: Apache-2.0
// Format-neutral content shared by the Markdown and HTML renderers, so both
// print the same labels and values.
import { stateLabel } from '../SourceState';
import { REPORT_SCHEMA, REPORT_VERSION, seriesStats, timeText } from './model';
import type { Cell, ChartSection, Column, Evidence, Report } from './model';

export type Item = { label: string; value: string };

/** Report-level metadata lines. */
export function headerItems(report: Report): Item[] {
  const { connection, range } = report;
  return [
    {
      label: 'Connection',
      value: `${connection.name} · ${connection.engine} · ${connection.database}`,
    },
    {
      label: 'Range',
      value: `${range.label} (${timeText(range.fromMs)} to ${timeText(range.toMs)})`,
    },
    { label: 'Generated', value: timeText(report.generatedMs) },
    { label: 'Schema', value: `${REPORT_SCHEMA} v${String(REPORT_VERSION)}` },
  ];
}

/** Source, scope and collection range of one section. */
export function evidenceItems(evidence: Evidence): Item[] {
  return [
    { label: 'Source', value: evidence.source },
    { label: 'Scope', value: evidence.scope },
    {
      label: 'Collected',
      value: `${timeText(evidence.fromMs)} to ${timeText(evidence.toMs)}`,
    },
  ];
}

/** Gaps heading text when a section reports none. */
export const NO_GAPS = 'None reported.';

/** Per-series latest/min/max/mean; unknown values stay `null`. */
export function chartStats(section: ChartSection): {
  columns: Column[];
  rows: Cell[][];
} {
  const unit = section.unit;
  return {
    columns: [
      { name: 'Series', unit: '' },
      { name: 'Latest', unit },
      { name: 'Latest state', unit: '' },
      { name: 'Min', unit },
      { name: 'Max', unit },
      { name: 'Mean', unit },
      { name: 'Observed', unit: '' },
      { name: 'Gaps', unit: '' },
    ],
    rows: section.series.map((series) => {
      const stats = seriesStats(series);
      return [
        series.name,
        stats.latest?.y ?? null,
        stats.latest ? stateLabel(stats.latest.state) : 'No observations',
        stats.min,
        stats.max,
        stats.mean,
        stats.observed,
        stats.gaps,
      ];
    }),
  };
}

/** Column header with its unit. */
export function columnTitle(column: Column): string {
  return column.unit ? `${column.name} (${column.unit})` : column.name;
}
