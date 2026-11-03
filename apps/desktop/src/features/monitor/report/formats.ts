// SPDX-License-Identifier: Apache-2.0
import type { saveReport } from '@openrows/ipc';
import { svgToPng } from '../../erd/rasterize';
import { columnTitle } from './content';
import { csvField } from './escape';
import { reportHtml } from './html';
import { reportMarkdown } from './markdown';
import { sectionRows, tabular } from './model';
import type { ChartSection, Report, ReportSection } from './model';
import { CHART_SIZE, chartSvg } from './svg';
import { t } from '../messages/lazyReport';

/** The file `saveReport` writes. */
export type ReportFile = Parameters<typeof saveReport>[1];
export type ReportFormat = 'markdown' | 'html' | 'json' | 'csv' | 'svg' | 'png';

export const FORMATS: readonly { value: ReportFormat; label: string }[] = [
  { value: 'markdown', label: t('markdown') },
  { value: 'html', label: t('html') },
  { value: 'json', label: t('json') },
  { value: 'csv', label: t('csvOneTable') },
  { value: 'svg', label: t('svgOneChart') },
  { value: 'png', label: t('pngOneChart') },
];

/** The versioned JSON document. */
export function reportJson(report: Report): string {
  return `${JSON.stringify(report, null, 2)}\n`;
}

/** CSV of one table section, or one row per chart observation. */
export function sectionCsv(section: ReportSection): string {
  if (!tabular(section)) return '';
  const { columns, rows } = sectionRows(section);
  const lines = [
    columns.map((column) => csvField(columnTitle(column))).join(','),
    ...rows.map((row) => row.map(csvField).join(',')),
  ];
  return `${lines.join('\r\n')}\r\n`;
}

/** Sections a format exports one of (CSV: tables and charts; images: charts). */
export function exportable(
  format: ReportFormat,
  sections: readonly ReportSection[],
): ReportSection[] {
  if (format === 'csv') return sections.filter(tabular);
  if (format === 'svg' || format === 'png')
    return sections.filter(
      (section): section is ChartSection => section.kind === 'chart',
    );
  return [...sections];
}

/** The file of `format`; `item` names the section of CSV/SVG/PNG exports. */
export async function reportFile(
  report: Report,
  format: ReportFormat,
  item: ReportSection | undefined,
): Promise<ReportFile> {
  switch (format) {
    case 'markdown':
      return { format, text: reportMarkdown(report) };
    case 'html':
      return { format, text: reportHtml(report) };
    case 'json':
      return { format, text: reportJson(report) };
    case 'csv':
      return { format, text: item ? sectionCsv(item) : '' };
    case 'svg':
    case 'png': {
      if (item?.kind !== 'chart') throw new Error(t('chooseChartExport'));
      const svg = chartSvg(item);
      if (format === 'svg') return { format, text: svg };
      const bytes = await svgToPng(svg, CHART_SIZE);
      return { format, bytes: Array.from(bytes) };
    }
  }
}
