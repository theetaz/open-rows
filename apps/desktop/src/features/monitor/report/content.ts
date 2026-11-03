// SPDX-License-Identifier: Apache-2.0
// Format-neutral content shared by the Markdown and HTML renderers, so both
// print the same labels and values.
import { stateLabel } from '../SourceState';
import { REPORT_SCHEMA, REPORT_VERSION, seriesStats, timeText } from './model';
import type { Cell, ChartSection, Column, Evidence, Report } from './model';
import { t } from '../messages/lazyReport';

export type Item = { label: string; value: string };

/** Report-level metadata lines. */
export function headerItems(report: Report): Item[] {
  const { connection, range } = report;
  return [
    {
      label: t('connection'),
      value: `${connection.name} · ${connection.engine} · ${connection.database}`,
    },
    {
      label: t('range'),
      value: t('rangeFromTo', {
        range: range.label,
        from: timeText(range.fromMs),
        to: timeText(range.toMs),
      }),
    },
    { label: t('generated'), value: timeText(report.generatedMs) },
    {
      label: t('schema'),
      value: `${REPORT_SCHEMA} v${String(REPORT_VERSION)}`,
    },
  ];
}

/** Source, scope and collection range of one section. */
export function evidenceItems(evidence: Evidence): Item[] {
  return [
    { label: t('source'), value: evidence.source },
    { label: t('scope'), value: evidence.scope },
    {
      label: t('collected'),
      value: t('timeFromTo', {
        from: timeText(evidence.fromMs),
        to: timeText(evidence.toMs),
      }),
    },
  ];
}

/** Gaps heading text when a section reports none. */
export const NO_GAPS = t('noneReported');

/** Per-series latest/min/max/mean; unknown values stay `null`. */
export function chartStats(section: ChartSection): {
  columns: Column[];
  rows: Cell[][];
} {
  const unit = section.unit;
  return {
    columns: [
      { name: t('series'), unit: '' },
      { name: t('latest'), unit },
      { name: t('latestState'), unit: '' },
      { name: t('min'), unit },
      { name: t('max'), unit },
      { name: t('mean'), unit },
      { name: t('observed'), unit: '' },
      { name: t('gaps'), unit: '' },
    ],
    rows: section.series.map((series) => {
      const stats = seriesStats(series);
      return [
        series.name,
        stats.latest?.y ?? null,
        stats.latest ? stateLabel(stats.latest.state) : t('noObservations'),
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
