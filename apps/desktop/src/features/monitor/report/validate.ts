// SPDX-License-Identifier: Apache-2.0
import type { Report } from './model';
import { REPORT_SCHEMA, REPORT_VERSION } from './model';
type Fields = Record<string, unknown>;
function object(value: unknown): Fields | null {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Fields)
    : null;
}
function text(value: unknown): boolean {
  return typeof value === 'string' && value.length <= 16384;
}
function number(value: unknown): boolean {
  return typeof value === 'number' && Number.isFinite(value);
}
function time(value: unknown): boolean {
  return (
    value === null ||
    (number(value) && Number(value) >= 0 && Number(value) <= 8640000000000000)
  );
}
function list(
  value: unknown,
  maximum: number,
  check: (item: unknown) => boolean,
): boolean {
  return (
    Array.isArray(value) &&
    value.length <= maximum &&
    (value as unknown[]).every(check)
  );
}
function cell(value: unknown): boolean {
  return value === null || text(value) || number(value);
}
function evidence(value: unknown): boolean {
  const data = object(value);
  return (
    !!data &&
    text(data.source) &&
    text(data.scope) &&
    time(data.fromMs) &&
    time(data.toMs) &&
    list(data.gaps, 100, text)
  );
}
function labelled(value: unknown): boolean {
  const data = object(value);
  return !!data && text(data.label) && text(data.value);
}
function column(value: unknown): boolean {
  const data = object(value);
  return !!data && text(data.name) && text(data.unit);
}
function section(value: unknown): boolean {
  const data = object(value);
  if (!data || !text(data.id) || !text(data.title) || !evidence(data.evidence))
    return false;
  switch (data.kind) {
    case 'summary':
      return list(data.items, 100, labelled);
    case 'stat':
      return cell(data.value);
    case 'table':
      return table(data);
    case 'chart':
      return chart(data);
    default:
      return false;
  }
}
function table(data: Fields): boolean {
  if (!list(data.columns, 200, column)) return false;
  const columns = data.columns as unknown[];
  return list(
    data.rows,
    10000,
    (row) =>
      Array.isArray(row) &&
      row.length === columns.length &&
      (row as unknown[]).every(cell),
  );
}
function point(value: unknown): boolean {
  const data = object(value);
  return (
    !!data &&
    number(data.x) &&
    text(data.label) &&
    text(data.state) &&
    (data.y === null || number(data.y))
  );
}
function series(value: unknown): boolean {
  const data = object(value);
  return !!data && text(data.name) && list(data.points, 10000, point);
}
function chart(data: Fields): boolean {
  return (
    ['line', 'area', 'bar', 'scatter'].includes(String(data.mark)) &&
    ['time', 'number', 'category'].includes(String(data.xKind)) &&
    text(data.unit) &&
    time(data.cadenceMs) &&
    list(data.series, 200, series)
  );
}
function connection(value: unknown): boolean {
  const data = object(value);
  return !!data && text(data.name) && text(data.engine) && text(data.database);
}
function range(value: unknown): boolean {
  const data = object(value);
  return !!data && time(data.fromMs) && time(data.toMs) && text(data.label);
}
function header(data: Fields): boolean {
  return (
    data.schema === REPORT_SCHEMA &&
    data.version === REPORT_VERSION &&
    text(data.title) &&
    number(data.generatedMs) &&
    time(data.generatedMs)
  );
}
/** Shared model boundary for deterministic reports and native validated proposals. */
export function validateReport(value: unknown): Report {
  const data = object(value);
  if (
    !data ||
    !header(data) ||
    !connection(data.connection) ||
    !range(data.range) ||
    !list(data.sections, 100, section) ||
    JSON.stringify(value).length > 8 * 1048576
  )
    throw new Error('Invalid report artifact');
  return value as Report;
}
/** Parse a bounded proposal before it reaches the shared report export tools. */
export function parseReport(document: string): Report {
  if (document.length > 65536) throw new Error('Report proposal is too large');
  return validateReport(JSON.parse(document) as unknown);
}
