// SPDX-License-Identifier: Apache-2.0
import type { CopyTable } from './formats';

function escapeHtml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

/**
 * An HTML table with a header row; NULL is written as `NULL` or left empty
 * and numeric columns are right-aligned, so spreadsheets and documents
 * paste it as a table.
 */
export function htmlTable(table: CopyTable, nulls: 'text' | 'empty'): string {
  const cell = (tag: 'th' | 'td', value: string | null, end: boolean) => {
    const text = value === null ? (nulls === 'text' ? 'NULL' : '') : value;
    const align = end ? ' style="text-align:right"' : '';
    return `<${tag}${align}>${escapeHtml(text).replace(/\r\n|\r|\n/g, '<br>')}</${tag}>`;
  };
  const numeric = table.columns.map((column) => column.kind === 'number');
  const header = table.columns
    .map((column) => cell('th', column.name, false))
    .join('');
  const rows = table.rows.map(
    (row) =>
      `<tr>${row.map((value, index) => cell('td', value, numeric[index] ?? false)).join('')}</tr>`,
  );
  return [
    '<table>',
    `<thead><tr>${header}</tr></thead>`,
    '<tbody>',
    ...rows,
    '</tbody>',
    '</table>',
  ].join('\n');
}
