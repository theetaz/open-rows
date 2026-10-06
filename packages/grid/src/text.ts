// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell } from './model';
/** Width measurement and string allocation happen on data arrival, never inside paint. */
export class TextCache {
  private readonly text = new WeakMap<DisplayCell, string>();
  prepare(
    context: CanvasRenderingContext2D,
    cell: DisplayCell,
    width: number,
  ): void {
    if (this.text.has(cell)) return;
    let end = cell.text.length;
    if (context.measureText(cell.text).width <= width) {
      this.text.set(cell, cell.text);
      return;
    }
    let low = 0;
    while (low < end) {
      const middle = Math.ceil((low + end) / 2);
      if (context.measureText(cell.text.slice(0, middle) + '…').width <= width)
        low = middle;
      else end = middle - 1;
    }
    end = low;
    this.text.set(cell, cell.text.slice(0, end) + '…');
  }
  /** A new uncached cell is clipped by the cell rectangle until preparation completes. */
  get(cell: DisplayCell): string {
    return this.text.get(cell) ?? cell.text;
  }
}
