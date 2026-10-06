// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import type { CellRange, GridColumn } from './model';

/** Prefix offsets are built once, rather than allocated in the paint loop. */
export class GridLayout {
  readonly offsets: Float64Array;
  readonly pinned: number;
  readonly pinnedWidth: number;
  readonly width: number;
  constructor(
    readonly columns: readonly GridColumn[],
    readonly rowHeight: number,
  ) {
    this.offsets = new Float64Array(columns.length + 1);
    let pinned = 0;
    columns.forEach((column, index) => {
      if (column.width <= 0 || !Number.isFinite(column.width))
        throw new GridError('layout', 'Invalid column width');
      if (column.pinned && pinned !== index)
        throw new GridError('layout', 'Pinned columns must lead');
      if (column.pinned) pinned++;
      this.offsets[index + 1] = (this.offsets[index] ?? 0) + column.width;
    });
    this.pinned = pinned;
    this.pinnedWidth = this.offsets[pinned] ?? 0;
    this.width = this.offsets[columns.length] ?? 0;
  }
  /** Binary search returns the column at an unscrolled content coordinate. */
  columnAt(x: number): number {
    let low = 0,
      high = this.columns.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if ((this.offsets[middle + 1] ?? 0) <= x) low = middle + 1;
      else high = middle;
    }
    return Math.min(low, this.columns.length - 1);
  }
  /** Fill a reusable range with one row/column margin, clamped to the source. */
  visible(target: CellRange, view: Viewport, rows: number): void {
    target.r0 = Math.max(0, Math.floor(view.y / this.rowHeight) - 1);
    target.r1 = Math.min(
      rows - 1,
      Math.ceil((view.y + view.height - this.rowHeight) / this.rowHeight),
    );
    target.c0 = Math.max(
      this.pinned,
      this.columnAt(view.x + this.pinnedWidth) - 1,
    );
    target.c1 = this.columnAt(view.x + view.width);
  }
  /** Hit testing respects the fixed header and pinned prefix. */
  hit(
    view: Viewport,
    x: number,
    y: number,
  ): { row: number; column: number } | undefined {
    if (y < this.rowHeight || x < 0) return undefined;
    return {
      row: Math.floor((y - this.rowHeight + view.y) / this.rowHeight),
      column: this.columnAt(x < this.pinnedWidth ? x : x + view.x),
    };
  }
}
/** CSS-pixel viewport; DPR only affects the backing store. */
export interface Viewport {
  x: number;
  y: number;
  width: number;
  height: number;
}
/** Snap strokes and text origins to physical pixels at fractional DPR. */
export function snap(value: number, dpr: number): number {
  return Math.round(value * dpr) / dpr;
}
/** Rounded backing size avoids stretching at noninteger DPR. */
export function backingSize(size: number, dpr: number): number {
  return Math.round(size * dpr);
}
