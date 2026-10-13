// SPDX-License-Identifier: Apache-2.0
import type { GridColumn } from '@openrows/grid';
import type { ColumnDto } from '@openrows/ipc';
import { columnAlign } from '../../results/align';
import { arrange, pinnedCount, widthOf } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import type { RowOrder } from './localRows';

/** Column identities for layouts: names, with repeats numbered (`id`, `id 2`). */
export function columnKeys(columns: readonly ColumnDto[]): string[] {
  const seen = new Map<string, number>();
  const reserved = new Set(columns.map((column) => column.name));
  const used = new Set<string>();
  return columns.map((column) => {
    let count = (seen.get(column.name) ?? 0) + 1;
    let key = count === 1 ? column.name : `${column.name} ${String(count)}`;
    while (used.has(key) || (count > 1 && reserved.has(key))) {
      count++;
      key = `${column.name} ${String(count)}`;
    }
    seen.set(column.name, count);
    used.add(key);
    return key;
  });
}

function defaultWidth(column: ColumnDto): number {
  return Math.min(320, Math.max(88, column.name.length * 8 + 40));
}

/** The client-side sort direction of a column, painted as a header triangle. */
function headerSort(
  sorted: RowOrder | null,
  index: number,
): Pick<GridColumn, 'sort'> {
  if (sorted?.column !== index) return {};
  return { sort: sorted.descending ? 'descending' : 'ascending' };
}

/**
 * Grid columns of a result in the user's order (pinned first) with stored
 * widths, and the result column behind each.
 */
export function resultColumns(
  columns: readonly ColumnDto[],
  layout: ColumnLayout,
  sorted: RowOrder | null,
): { columns: GridColumn[]; order: number[] } {
  const keys = columnKeys(columns);
  const shown = arrange(layout, keys);
  const pins = pinnedCount(layout, shown);
  const order = shown.map((key) => keys.indexOf(key));
  return {
    order,
    columns: order.map((index, position) => {
      const column = columns[index];
      const key = keys[index] ?? '';
      return {
        id: String(index),
        title: column?.name ?? '',
        ...headerSort(sorted, index),
        width: widthOf(layout, key, column ? defaultWidth(column) : 88),
        pinned: position < pins,
        align: columnAlign(column?.sourceType ?? null),
      };
    }),
  };
}
