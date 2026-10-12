// SPDX-License-Identifier: Apache-2.0
// Escaping for database-provided text (query text, names, values) in every
// export format. Nothing from a database reaches markup unescaped.

const HTML: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
};

/** Text and attribute-safe HTML/SVG/XML escaping. */
export function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (char) => HTML[char] ?? char);
}

/**
 * Markdown text inside a list item or table cell: raw HTML is entity-escaped,
 * inline syntax (emphasis, code, links, images, cells) is backslash-escaped
 * and line breaks become spaces so a value cannot start a new block. Values
 * never begin a line, so block markers such as `#` or `-` stay literal.
 */
export function escapeMarkdown(text: string): string {
  return text
    .replace(/[&<>]/g, (char) => HTML[char] ?? char)
    .replace(/[\\`*_{}[\]|~]/g, (char) => `\\${char}`)
    .replace(/\r\n|\r|\n/g, ' ');
}

const PLAIN_NUMBER = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;
/** Leading characters spreadsheets evaluate as a formula. */
const FORMULA = /^[=+\-@\t\r]/;

/**
 * One RFC 4180 CSV field. Text a spreadsheet would run as a formula gets a
 * leading `'`; plain numbers (including negative ones) stay numbers.
 */
export function csvField(value: string | number | null): string {
  if (value === null) return '';
  if (typeof value === 'number') return String(value);
  const guarded =
    FORMULA.test(value) && !PLAIN_NUMBER.test(value) ? `'${value}` : value;
  return /[",\r\n]/.test(guarded)
    ? `"${guarded.replace(/"/g, '""')}"`
    : guarded;
}
