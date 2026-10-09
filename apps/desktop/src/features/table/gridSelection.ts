// SPDX-License-Identifier: Apache-2.0
import type { CellRange } from '@openrows/grid';
import type { ResultMeta } from '../query/run';
import type { Cell } from './bulk/cells';
import type { CopySource } from '../../results/copy/read';

/** The table grid's selection in result (data) coordinates. */
export type GridSelection = {
  result: ResultMeta;
  /** The focused cell. */
  cell: Cell;
  ranges: readonly CellRange[];
};

/**
 * What a copy of the selection reads: the selected cells, or the focused row
 * when nothing is range-selected. Staged inserts have no stored row and are
 * left out.
 */
export function copySource({
  result,
  cell,
  ranges,
}: GridSelection): CopySource {
  const chosen: readonly CellRange[] = ranges.length
    ? ranges
    : [{ r0: cell.row, r1: cell.row, c0: 0, c1: result.columns.length - 1 }];
  return {
    result,
    ranges: chosen.map((range) => ({
      ...range,
      r1: Math.min(range.r1, result.rows - 1),
    })),
  };
}
