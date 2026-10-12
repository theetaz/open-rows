// SPDX-License-Identifier: Apache-2.0
import {
  chartStats,
  columnTitle,
  evidenceItems,
  headerItems,
  NO_GAPS,
} from './content';
import type { Item } from './content';
import { escapeMarkdown as md } from './escape';
import { cellText } from './model';
import type { Cell, Column, Report, ReportSection } from './model';

function list(items: Item[]): string[] {
  return items.map((item) => `- **${md(item.label)}:** ${md(item.value)}`);
}

function table(columns: Column[], rows: Cell[][]): string[] {
  if (!columns.length) return [];
  const row = (cells: string[]): string => `| ${cells.join(' | ')} |`;
  return [
    row(columns.map((column) => md(columnTitle(column)))),
    row(columns.map(() => '---')),
    ...rows.map((cells) => row(cells.map((cell) => md(cellText(cell))))),
  ];
}

function body(section: ReportSection): string[] {
  switch (section.kind) {
    case 'summary':
      return list(section.items);
    case 'stat':
      return [`**${md(cellText(section.value))}**`];
    case 'table':
      return section.rows.length
        ? table(section.columns, section.rows)
        : ['No rows.'];
    case 'chart': {
      const stats = chartStats(section);
      return [
        ...table(stats.columns, stats.rows),
        '',
        'Every observation is in the JSON and CSV exports; the chart is in the HTML, SVG and PNG exports.',
      ];
    }
  }
}

function section(part: ReportSection): string[] {
  const gaps = part.evidence.gaps;
  return [
    `## ${md(part.title)}`,
    '',
    ...list(evidenceItems(part.evidence)),
    `- **Gaps and unknowns:** ${gaps.length ? '' : NO_GAPS}`,
    ...gaps.map((gap) => `  - ${md(gap)}`),
    '',
    ...body(part),
    '',
  ];
}

/** Markdown document; every database-provided string is escaped. */
export function reportMarkdown(report: Report): string {
  return [
    `# ${md(report.title)}`,
    '',
    ...list(headerItems(report)),
    '',
    ...report.sections.flatMap(section),
  ].join('\n');
}
