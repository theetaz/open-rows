// SPDX-License-Identifier: Apache-2.0
import type { ColumnChunk } from './tiles';

/** Formats one element of `chunk` at `row`, or `null` for SQL NULL. */
type ItemText = (
  chunk: ColumnChunk,
  type: string,
  row: number,
) => string | null;

/** The element type of a `List<…>` type string. */
export function itemType(type: string): string {
  return type.slice('List<'.length, -1);
}

/**
 * `array_out` quoting: empty items, `NULL`, and items with braces, commas,
 * quotes, backslashes or white space are double-quoted with `"` and `\` escaped.
 */
export function arrayItem(text: string): string {
  const plain =
    text !== '' && text.toUpperCase() !== 'NULL' && !/[{}",\\\s]/.test(text);
  return plain ? text : `"${text.replace(/["\\]/g, '\\$&')}"`;
}

/** A list value as `PostgreSQL` array text, e.g. `{1,NULL,"a b"}`. */
export function listValue(
  chunk: ColumnChunk,
  type: string,
  row: number,
  item: ItemText,
): string {
  const { child, offsets } = chunk;
  if (!child || !offsets) return '{}';
  const start = offsets[row] ?? 0;
  const end = offsets[row + 1] ?? start;
  const items: string[] = [];
  for (let index = start; index < end; index++) {
    const text = item(child, itemType(type), index);
    items.push(text === null ? 'NULL' : arrayItem(text));
  }
  return `{${items.join(',')}}`;
}
