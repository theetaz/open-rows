// SPDX-License-Identifier: Apache-2.0
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import { TileSource } from '@openrows/grid';
import { emptyLayout } from '../../results/columnLayout';
import type { ColumnLayout } from '../../results/columnLayout';
import { displayWindow } from '../../results/displayWindow';
import type { ResultMeta } from './run';
import type { ResultView } from './useResultView';
import { columnKeys, resultColumns } from './resultColumns';
import { ResultViewSource } from './ResultViewSource';

/** Give all table tools the same displayed coordinates, retaining hidden key columns. */
export function projectResult(args: {
  result: ResultMeta;
  layout: Pick<ColumnLayout, 'order' | 'pinned'>;
  shown: ResultView['shown'];
  hidden: readonly string[];
}): ResultMeta {
  const { result, layout, shown, hidden } = args;
  const keys = columnKeys(result.columns);
  const arrangement = resultColumns(
    result.columns,
    { ...emptyLayout, ...layout },
    null,
  );
  const visible = arrangement.order.filter(
    (index) => !hidden.includes(keys[index] ?? ''),
  );
  const order = [
    ...visible,
    ...arrangement.order.filter((index) => !visible.includes(index)),
  ];
  const source = new ResultViewSource(
    new TileSource(
      result.id,
      result.rows,
      result.columns.map((column, index) => ({
        id: String(index),
        title: column.name,
        width: 100,
        pinned: false,
      })),
      displayWindow,
    ),
  );
  source.arrange(
    order.map((index) => ({
      id: String(index),
      title: result.columns[index]?.name ?? '',
      width: 100,
      pinned: false,
    })),
    order,
  );
  source.show(shown);
  return {
    ...result,
    rows: source.rows,
    columnKeys: order.map((index) => keys[index] ?? ''),
    columns: visible.flatMap((index) =>
      result.columns[index] ? [result.columns[index]] : [],
    ),
    hidden: order
      .slice(visible.length)
      .flatMap((index) =>
        result.columns[index] ? [result.columns[index]] : [],
      ),
    projection: {
      rowOf: (row) => source.rowOf(row),
      columnOf: (column) => source.columnOf(column),
      load: (request) => projectedWindow(source, request),
    },
  };
}

async function projectedWindow(
  source: ResultViewSource,
  request: WindowRequest,
): Promise<DisplayTile> {
  const { rowStart, rowCount, colStart, colCount } = request;
  await source.window(
    {
      r0: rowStart,
      r1: rowStart + rowCount - 1,
      c0: colStart,
      c1: colStart + colCount - 1,
    },
    0,
  );
  const cells = [];
  for (let row = rowStart; row < rowStart + rowCount; row++)
    for (let column = colStart; column < colStart + colCount; column++) {
      const cell = source.cell(row, column);
      if (!cell)
        throw new Error('The selected result window is no longer available.');
      cells.push(cell);
    }
  return { cells, columns: colCount, convertMs: 0 };
}
