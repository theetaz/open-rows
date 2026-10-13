// SPDX-License-Identifier: Apache-2.0
import type { WindowRequest } from '@openrows/ipc';
import type { DisplayTile } from '@openrows/grid/display-tile';
import type { ResultMeta } from '../features/query/run';
import { displayWindow } from './displayWindow';

/** Result views read through their coordinate adapter; stored results use Arrow IPC. */
export function readResultWindow(
  result: ResultMeta,
  request: WindowRequest,
): Promise<DisplayTile> {
  return result.projection
    ? result.projection.load(request)
    : displayWindow(request);
}

/** Stored coordinates for full-value inspection and export. */
export function storedRow(result: ResultMeta, row: number): number {
  return result.projection?.rowOf(row) ?? row;
}
export function storedColumn(result: ResultMeta, column: number): number {
  return result.projection?.columnOf(column) ?? column;
}

/** Layout/filter identity; database column labels remain unchanged for reads and edits. */
export function resultColumnKey(result: ResultMeta, column: number): string {
  return (
    result.columnKeys?.[column] ??
    result.columns[column]?.name ??
    result.hidden?.[column - result.columns.length]?.name ??
    ''
  );
}
