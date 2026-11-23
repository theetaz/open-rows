// SPDX-License-Identifier: Apache-2.0
import { rowTop, snap } from './layout';
import type { CellRange } from './model';
import type { CanvasPainter } from './painter';

/** A header or Ctrl/Cmd+Space column selection covers every row of one span. */
function columnRange(range: CellRange, rows: number): boolean {
  return rows > 1 && range.r0 === 0 && range.r1 >= rows - 1;
}

/** TablePlus-style selection: selected rows take the accent tint, columns tint their cells. */
export class SelectionPainter {
  constructor(private readonly painter: CanvasPainter) {}
  /** Repaint selected cells over the body, then ring the focused cell. */
  paint(context: CanvasRenderingContext2D): void {
    const { selection } = this.painter.props;
    if (!selection.ranges.length) return;
    this.region(context, true);
    this.region(context, false);
    this.ring(context);
  }
  private region(context: CanvasRenderingContext2D, pinned: boolean): void {
    const { range, layout } = this.painter;
    const first = pinned ? 0 : range.c0,
      last = pinned ? layout.pinned - 1 : range.c1;
    if (first > last) return;
    context.save();
    this.painter.clipRegion(context, pinned);
    for (let row = range.r0; row <= range.r1; row++) {
      const whole = this.rowSelected(row);
      for (let col = first; col <= last; col++)
        if (whole || this.cellSelected(row, col))
          this.painter.cell(context, row, col, pinned, true);
    }
    context.restore();
  }
  private rowSelected(row: number): boolean {
    const { selection, source } = this.painter.props;
    for (const range of selection.ranges)
      if (
        row >= range.r0 &&
        row <= range.r1 &&
        !columnRange(range, source.rows)
      )
        return true;
    return false;
  }
  private cellSelected(row: number, col: number): boolean {
    for (const range of this.painter.props.selection.ranges)
      if (
        row >= range.r0 &&
        row <= range.r1 &&
        col >= range.c0 &&
        col <= range.c1
      )
        return true;
    return false;
  }
  /** The focused cell: a 2 px accent ring inside the cell, visible over any tint. */
  private ring(context: CanvasRenderingContext2D): void {
    const { props, layout, view, dpr } = this.painter;
    const { row, column } = props.selection,
      { colours } = props.theme,
      { rowHeight } = layout;
    const pinned = column < layout.pinned;
    const x = snap((layout.offsets[column] ?? 0) - (pinned ? 0 : view.x), dpr);
    const y = snap(rowTop(layout, row) - view.y, dpr),
      width = props.source.columns[column]?.width ?? 0;
    context.save();
    this.painter.clipRegion(context, pinned);
    context.strokeStyle = colours['accent-fill'];
    context.lineWidth = 2;
    context.strokeRect(x + 1, y + 1, width - 2, rowHeight - 2);
    context.restore();
  }
}
