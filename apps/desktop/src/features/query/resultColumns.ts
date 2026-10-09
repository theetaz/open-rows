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
  return columns.map((column) => {
    const count = (seen.get(column.name) ?? 0) + 1;
    seen.set(column.name, count);
    return count === 1 ? column.name : `${column.name} ${String(count)}`;
  });
}

function defaultWidth(column: ColumnDto): number {
  return Math.min(320, Math.max(88, column.name.length * 8 + 40));
}

/** Header text with the client-side sort direction, e.g. `name ▲`. */
function title(name: string, sorted: RowOrder | null, index: number): string {
  if (sorted?.column !== index) return name;
  return `${name} ${sorted.descending ? '▼' : '▲'}`;
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
        title: title(column?.name ?? '', sorted, index),
        width: widthOf(layout, key, column ? defaultWidth(column) : 88),
        pinned: position < pins,
        align: columnAlign(column?.sourceType ?? null),
      };
    }),
  };
}
