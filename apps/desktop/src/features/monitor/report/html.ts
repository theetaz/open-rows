// SPDX-License-Identifier: Apache-2.0
// Self-contained HTML: inline style only, no scripts, event handlers or
// external resources; a CSP forbids them even if a reader injects markup.
import {
  chartStats,
  columnTitle,
  evidenceItems,
  headerItems,
  NO_GAPS,
} from './content';
import type { Item } from './content';
import { escapeHtml as h } from './escape';
import { cellText } from './model';
import type { Cell, Column, Report, ReportSection } from './model';
import { chartSvg } from './svg';

const CSP = "default-src 'none'; style-src 'unsafe-inline'";
const STYLE = `body{font:13px -apple-system,system-ui,sans-serif;color:#18181B;background:#fff;margin:24px;max-width:960px}
h1{font-size:18px;margin:0 0 8px}h2{font-size:14px;margin:24px 0 6px;padding-top:12px;border-top:1px solid #E4E4E7}
dl{display:grid;grid-template-columns:max-content 1fr;gap:2px 12px;margin:0 0 8px;color:#52525B}dt{font-weight:600}dd{margin:0}
table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums;margin:6px 0}
th,td{text-align:left;padding:3px 8px;border-bottom:1px solid #E4E4E7;vertical-align:top;overflow-wrap:anywhere}
th{color:#52525B;font-weight:600}.gaps{color:#52525B}.stat{font-size:20px}svg{max-width:100%;height:auto}`;

function items(list: Item[]): string {
  return `<dl>${list.map((item) => `<dt>${h(item.label)}</dt><dd>${h(item.value)}</dd>`).join('')}</dl>`;
}

function table(columns: Column[], rows: Cell[][]): string {
  const head = columns
    .map((column) => `<th scope="col">${h(columnTitle(column))}</th>`)
    .join('');
  const body = rows
    .map(
      (row) =>
        `<tr>${row.map((cell) => `<td>${h(cellText(cell))}</td>`).join('')}</tr>`,
    )
    .join('');
  return `<table><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table>`;
}

function body(section: ReportSection): string {
  switch (section.kind) {
    case 'summary':
      return items(section.items);
    case 'stat':
      return `<p class="stat">${h(cellText(section.value))}</p>`;
    case 'table':
      return section.rows.length
        ? table(section.columns, section.rows)
        : '<p>No rows.</p>';
    case 'chart': {
      const stats = chartStats(section);
      return chartSvg(section) + table(stats.columns, stats.rows);
    }
  }
}

function gaps(list: string[]): string {
  if (!list.length) return `<p class="gaps">Gaps and unknowns: ${NO_GAPS}</p>`;
  return `<p class="gaps">Gaps and unknowns:</p><ul class="gaps">${list.map((gap) => `<li>${h(gap)}</li>`).join('')}</ul>`;
}

function section(part: ReportSection): string {
  return `<section><h2>${h(part.title)}</h2>${items(evidenceItems(part.evidence))}${gaps(part.evidence.gaps)}${body(part)}</section>`;
}

/** Self-contained, script-free HTML document of the report. */
export function reportHtml(report: Report): string {
  return [
    '<!DOCTYPE html>',
    '<html lang="en"><head><meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${CSP}">`,
    `<title>${h(report.title)}</title><style>${STYLE}</style></head><body>`,
    `<h1>${h(report.title)}</h1>${items(headerItems(report))}`,
    report.sections.map(section).join(''),
    '</body></html>',
  ].join('\n');
}
