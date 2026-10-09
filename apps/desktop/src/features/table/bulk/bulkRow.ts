// SPDX-License-Identifier: Apache-2.0
import type { CellRange } from '@openrows/grid';
import type { CellValue } from '../cellValue';
import type { RowField } from '../row';
import { selectedRows } from './cells';

/** Most rows the bulk inspector reads to compare values. */
export const BULK_READ_LIMIT = 1_000;

/** A field across several rows: their common value, or several values. */
export type BulkField = {
  name: string;
  type: string | null;
  /** The value every row shares (`null` is SQL NULL); absent when they differ. */
  common?: string | null;
  /** Some row has this field staged. */
  changed: boolean;
};

/** Several selected rows in the inspector (FR-GRID-14). */
export type BulkRows = {
  /** Rows a staged value applies to. */
  rows: number;
  fields: readonly BulkField[];
  /** Stage `value` in this field of every selected row; absent when read-only. */
  stage?: (name: string, value: CellValue) => void;
};

/** Selected page rows (staged inserts excluded), ascending. */
export function pageRows(ranges: readonly CellRange[], rows: number): number[] {
  return selectedRows(ranges).filter((row) => row < rows);
}

/** One range per run of consecutive rows, on one column. */
export function columnRanges(
  rows: readonly number[],
  column: number,
): CellRange[] {
  const ranges: CellRange[] = [];
  for (const row of rows) {
    const last = ranges.at(-1);
    if (last && last.r1 === row - 1) last.r1 = row;
    else ranges.push({ r0: row, r1: row, c0: column, c1: column });
  }
  return ranges;
}

const valueOf = (field: RowField): string | null =>
  field.state === 'null' ? null : field.state === 'empty' ? '' : field.text;

/** Each shown field with the value all `rows` share, if any. */
export function bulkFields(
  rows: readonly (readonly RowField[])[],
): BulkField[] {
  const [first, ...rest] = rows;
  if (!first) return [];
  return first.map((field, index) => {
    const value = valueOf(field);
    const same = rest.every((row) => {
      const other = row[index];
      return other !== undefined && valueOf(other) === value;
    });
    const changed = rows.some((row) => row[index]?.state === 'changed');
    const bulk: BulkField = { name: field.name, type: field.type, changed };
    if (same) bulk.common = value;
    return bulk;
  });
}
