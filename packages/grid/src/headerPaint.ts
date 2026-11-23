// SPDX-License-Identifier: Apache-2.0
import { snap } from './layout';
import type { GridColumn } from './model';
import type { CanvasPainter } from './painter';

/** Header room reserved for the sort triangle. */
const SORT_ROOM = 18;
/** Inset of the title from the column edges, and the gap before the type. */
const INSET = 10;
const CAPTION_GAP = 6;

/** A small filled ▲/▼ centred on (cx, cy), right-aligned in the header like TablePlus. */
function sortTriangle(
  context: CanvasRenderingContext2D,
  at: { cx: number; cy: number; up: boolean },
): void {
  const { cx, cy } = at;
  const base = at.up ? cy + 2 : cy - 2,
    apex = at.up ? cy - 2.5 : cy + 2.5;
  context.beginPath();
  context.moveTo(cx - 3.5, base);
  context.lineTo(cx + 3.5, base);
  context.lineTo(cx, apex);
  context.closePath();
  context.fill();
}

/**
 * The sticky header (v4 grid): an opaque `grid-head` band, each column's
 * name in semibold secondary followed by its type in a smaller tertiary
 * caption, column rules and a hairline below. Never allocates per frame
 * beyond the canvas' own text measurement.
 */
export class HeaderPainter {
  /** Measured label widths per font, so scrolling never re-measures. */
  private readonly widths = new Map<string, Map<string, number>>();
  constructor(private readonly painter: CanvasPainter) {}
  private width(context: CanvasRenderingContext2D, text: string): number {
    let fonts = this.widths.get(context.font);
    if (!fonts) {
      fonts = new Map();
      this.widths.set(context.font, fonts);
    }
    let width = fonts.get(text);
    if (width === undefined) {
      width = context.measureText(text).width;
      fonts.set(text, width);
    }
    return width;
  }
  paint(context: CanvasRenderingContext2D): void {
    const { painter } = this;
    const { colours } = painter.props.theme,
      height = painter.layout.headerHeight;
    context.save();
    for (const pinned of [false, true]) {
      context.fillStyle = colours['grid-head'];
      context.fillRect(
        0,
        0,
        pinned ? painter.layout.pinnedWidth : painter.view.width,
        height,
      );
      const first = pinned ? 0 : painter.range.c0,
        last = pinned ? painter.layout.pinned - 1 : painter.range.c1;
      for (let col = first; col <= last; col++) this.cell(context, col, pinned);
    }
    context.fillStyle = colours.separator;
    const hair = 1 / painter.dpr;
    context.fillRect(0, height - hair, painter.view.width, hair);
    context.restore();
  }
  private cell(
    context: CanvasRenderingContext2D,
    col: number,
    pinned: boolean,
  ): void {
    const { painter } = this;
    const column = painter.props.source.columns[col];
    if (!column) return;
    const { colours } = painter.props.theme,
      height = painter.layout.headerHeight;
    const x =
      (painter.layout.offsets[col] ?? 0) - (pinned ? 0 : painter.view.x);
    const arrow = column.sort ? SORT_ROOM : 0;
    context.save();
    context.beginPath();
    context.rect(x, 0, column.width - arrow, height);
    context.clip();
    this.labels(context, column, { x, width: column.width - arrow });
    context.restore();
    const hair = 1 / painter.dpr;
    context.fillStyle = colours['grid-line'];
    context.fillRect(x + column.width - hair, 0, hair, height);
    context.fillStyle = colours.secondary;
    if (column.sort)
      sortTriangle(context, {
        cx: x + column.width - SORT_ROOM / 2,
        cy: height / 2,
        up: column.sort === 'ascending',
      });
  }
  /** Measure the visible titles and captions outside the paint callback. */
  prepare(context: CanvasRenderingContext2D): void {
    const { painter } = this,
      { theme, source } = painter.props;
    const font = context.font;
    for (let col = 0; col <= painter.range.c1; col++) {
      const column = source.columns[col];
      if (!column || (col < painter.range.c0 && col >= painter.layout.pinned))
        continue;
      context.font = theme.headerFont ?? font;
      this.width(context, column.title);
      context.font = theme.captionFont ?? context.font;
      if (column.caption) this.width(context, column.caption);
    }
    context.font = font;
  }
  /**
   * Title then type caption: start-aligned, or end-aligned for numbers
   * (the caption at the end, the title before it).
   */
  private labels(
    context: CanvasRenderingContext2D,
    column: GridColumn,
    box: { x: number; width: number },
  ): void {
    const { theme } = this.painter.props,
      { colours } = theme,
      dpr = this.painter.dpr,
      y = this.painter.layout.headerHeight / 2;
    const titleFont = theme.headerFont ?? context.font;
    const captionFont = theme.captionFont ?? titleFont;
    const caption = column.caption ?? '';
    const end = column.align === 'end';
    context.textAlign = end ? 'right' : 'left';
    let at = end ? box.x + box.width - INSET : box.x + INSET;
    if (end && caption) {
      context.font = captionFont;
      context.fillStyle = colours.tertiary;
      context.fillText(caption, snap(at, dpr), y);
      at -= this.width(context, caption) + CAPTION_GAP;
    }
    context.font = titleFont;
    context.fillStyle = column.sort ? colours.text : colours.secondary;
    context.fillText(column.title, snap(at, dpr), y);
    if (end || !caption) return;
    at += this.width(context, column.title) + CAPTION_GAP;
    context.font = captionFont;
    context.fillStyle = colours.tertiary;
    context.fillText(caption, snap(at, dpr), y);
  }
}
