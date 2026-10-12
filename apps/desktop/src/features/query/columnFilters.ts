// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell } from '@openrows/grid';
import type { QuickOperator } from '../table/filters/QuickFilter';

type ColumnOperator =
  'contains' | 'equal' | 'notEqual' | 'isNull' | 'isNotNull';

/** A header quick filter on one result column, applied to loaded rows. */
export type ColumnFilter = {
  column: number;
  operator: ColumnOperator;
  value: string;
};

/** Operators of a result column's quick filter (client-side, FR-GRID-11). */
export const COLUMN_OPERATORS: readonly QuickOperator<ColumnOperator>[] = [
  { value: 'contains', label: 'contains', arity: 1 },
  { value: 'equal', label: '=', arity: 1 },
  { value: 'notEqual', label: '≠', arity: 1 },
  { value: 'isNull', label: 'IS NULL', arity: 0 },
  { value: 'isNotNull', label: 'IS NOT NULL', arity: 0 },
];

/**
 * Whether a cell passes a filter: `contains` ignores case, `=`/`≠` compare
 * the shown text exactly and, as in SQL, NULL never compares.
 */
export function cellMatches(
  cell: DisplayCell | undefined,
  filter: ColumnFilter,
): boolean {
  const isNull = !cell || cell.state === 'null';
  if (filter.operator === 'isNull') return isNull;
  if (filter.operator === 'isNotNull' || isNull) return !isNull;
  const text = cell.state === 'empty' ? '' : cell.text;
  switch (filter.operator) {
    case 'contains':
      return text.toLowerCase().includes(filter.value.toLowerCase());
    case 'equal':
      return text === filter.value;
    case 'notEqual':
      return text !== filter.value;
  }
}

/** The filters with `filter` replacing any earlier one on its column. */
export function withColumnFilter(
  filters: readonly ColumnFilter[],
  filter: ColumnFilter,
): ColumnFilter[] {
  return [...filters.filter((item) => item.column !== filter.column), filter];
}
