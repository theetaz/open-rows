// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import { GridLayout, rowTop, snap, backingSize } from './layout';
import type { Viewport } from './layout';
import type { CellRange, GridProps, PaintMetrics } from './model';
import { TextCache } from './text';
import { paintMetrics } from './metrics';
import { CellPainter, textWidth } from './cellPaint';
import { SelectionPainter } from './selectionPaint';

/** Four canvas layers use persistent contexts and reusable ranges. */
export class CanvasPainter {
  readonly view: Viewport = { x: 0, y: 0, width: 0, height: 0 };
  readonly range: CellRange = { r0: 0, r1: 0, c0: 0, c1: 0 };
  layout: GridLayout;
  readonly metrics: PaintMetrics = paintMetrics();
  private readonly contexts: CanvasRenderingContext2D[];
  private text = new TextCache();
  private dirty = true;
  private previousX = -1;
  private previousY = -1;
  private selectionModel: GridProps['selection'] | undefined;
  private readonly cells: CellPainter;
  private readonly selected = new SelectionPainter(this);
  dpr = window.devicePixelRatio || 1;
  constructor(
    host: HTMLElement,
    readonly props: GridProps,
  ) {
    this.layout = layoutFor(props);
    this.cells = new CellPainter(props.theme.colours);
    this.cells.dpr = this.dpr;
    this.contexts = ['body', 'pinned', 'header', 'selection'].map((layer) => {
      const canvas = document.createElement('canvas');
      canvas.dataset.layer = layer;
      canvas.setAttribute('aria-hidden', 'true');
      canvas.style.cssText = 'position:absolute;inset:0;pointer-events:none;';
      host.append(canvas);
      const context = canvas.getContext('2d');
      if (!context) throw new GridError('canvas', 'Canvas 2D unavailable');
      return context;
    });
  }
  /** Invalidate font measurements outside painting and retain the viewport. */
  setTheme(theme: GridProps['theme']): void {
    const previous = this.props.theme;
    const metricsChanged =
      previous.fontSize !== theme.fontSize ||
      previous.fontFamily !== theme.fontFamily ||
      previous.rowHeight !== theme.rowHeight ||
      previous.headerHeight !== theme.headerHeight;
    this.props.theme = theme;
    this.cells.setColours(theme.colours);
    if (metricsChanged) {
      this.layout = layoutFor(this.props);
      this.text = new TextCache();
      for (const context of this.contexts)
        context.font = `${String(theme.fontSize)}px ${theme.fontFamily}`;
    }
    this.prepare();
  }
  /** Column widths, order or pins changed: rebuild offsets and re-measure. */
  relayout(): void {
    this.layout = layoutFor(this.props);
    this.refresh();
  }
  /** Text width in the cell font; for auto-size, outside painting. */
  readonly measure = (text: string): number =>
    this.contexts[0]?.measureText(text).width ?? 0;
  /** Column kinds changed: re-measure truncated text against the new widths. */
  refresh(): void {
    this.text = new TextCache();
    this.prepare();
  }
  /** Resize only on host geometry changes, avoiding per-frame backing allocations. */
  resize(width: number, height: number): void {
    const dpr = window.devicePixelRatio || 1;
    if (
      width === this.view.width &&
      height === this.view.height &&
      dpr === this.dpr
    )
      return;
    const started = performance.now();
    this.dpr = dpr;
    this.cells.dpr = dpr;
    this.dirty = true;
    this.view.width = width;
    this.view.height = height;
    for (const context of this.contexts) {
      const backingWidth = backingSize(width, this.dpr),
        backingHeight = backingSize(height, this.dpr);
      if (context.canvas.width !== backingWidth) {
        context.canvas.width = backingWidth;
        this.metrics.canvasResizeEvents++;
      }
      if (context.canvas.height !== backingHeight) {
        context.canvas.height = backingHeight;
        this.metrics.canvasResizeEvents++;
      }
      context.canvas.style.width = `${String(width)}px`;
      context.canvas.style.height = `${String(height)}px`;
      context.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
      context.font = `${String(this.props.theme.fontSize)}px ${this.props.theme.fontFamily}`;
      context.textBaseline = 'middle';
    }
    this.metrics.canvasResizeMs += performance.now() - started;
  }
  /** Prepare widths outside the animation callback after tiles arrive or the window moves. */
  prepare(): void {
    const started = performance.now();
    this.dirty = true;
    const context = this.contexts[0];
    if (!context) return;
    this.layout.visible(this.range, this.view, this.props.source.rows);
    for (let row = this.range.r0; row <= this.range.r1; row++) {
      for (let col = 0; col <= this.range.c1; col++) {
        if (col >= this.range.c0 || col < this.layout.pinned) {
          const cell = this.props.source.cell(row, col);
          if (cell)
            this.text.prepare(
              context,
              cell,
              textWidth(this.props.source.columns[col]),
            );
        }
      }
    }
    this.metrics.prepareMs += performance.now() - started;
  }
  /** Timed complete paint, with no arrays, objects or strings allocated in the hot loop. */
  paint(): void {
    const started = performance.now();
    this.layout.visible(this.range, this.view, this.props.source.rows);
    const horizontal = this.previousX !== this.view.x,
      vertical = this.previousY !== this.view.y;
    const moved = this.viewportChanged();
    if (this.dirty || moved) {
      this.clear(0);
      this.body(false);
    }
    if (this.dirty || vertical) {
      this.clear(1);
      this.body(true);
    }
    if (this.dirty || horizontal) {
      this.clear(2);
      this.header();
    }
    if (this.dirty || moved || this.selectionModel !== this.props.selection) {
      this.clear(3);
      const context = this.contexts[3];
      if (context) this.selected.paint(context);
    }
    this.previousX = this.view.x;
    this.previousY = this.view.y;
    this.selectionModel = this.props.selection;
    this.dirty = false;
    this.metrics.milliseconds += performance.now() - started;
    this.metrics.paints++;
  }
  private viewportChanged(): boolean {
    return this.previousX !== this.view.x || this.previousY !== this.view.y;
  }
  private clear(index: number): void {
    this.contexts[index]?.clearRect(0, 0, this.view.width, this.view.height);
  }
  private body(pinned: boolean): void {
    const context = this.contexts[pinned ? 1 : 0];
    if (!context) return;
    const left = pinned ? 0 : this.layout.pinnedWidth,
      right = pinned ? this.layout.pinnedWidth : this.view.width;
    context.save();
    this.clipRegion(context, pinned);
    context.fillStyle = this.props.theme.colours.window;
    context.fillRect(left, 0, right - left, this.view.height);
    const first = pinned ? 0 : this.range.c0,
      last = pinned ? this.layout.pinned - 1 : this.range.c1;
    for (let row = this.range.r0; row <= this.range.r1; row++) {
      for (let col = first; col <= last; col++)
        this.cell(context, row, col, pinned);
    }
    context.restore();
  }
  /** Clip to the pinned prefix or the scrolled body, below the header row. */
  clipRegion(context: CanvasRenderingContext2D, pinned: boolean): void {
    const left = pinned ? 0 : this.layout.pinnedWidth,
      right = pinned ? this.layout.pinnedWidth : this.view.width;
    context.beginPath();
    context.rect(
      left,
      this.layout.headerHeight,
      right - left,
      this.view.height,
    );
    context.clip();
  }
  /** Paint one cell; selected cells take the accent tint instead of row states. */
  cell(
    context: CanvasRenderingContext2D,
    row: number,
    col: number,
    pinned: boolean,
    selected = false,
  ): void {
    const { theme, source } = this.props,
      box = this.cells.box;
    box.x = snap(
      (this.layout.offsets[col] ?? 0) - (pinned ? 0 : this.view.x),
      this.dpr,
    );
    box.y = snap(rowTop(this.layout, row) - this.view.y, this.dpr);
    box.width = source.columns[col]?.width ?? 0;
    box.height = theme.rowHeight;
    const cell = source.cell(row, col);
    context.save();
    context.beginPath();
    context.rect(box.x, box.y, box.width, box.height);
    context.clip();
    this.fill(context, row, selected);
    if (!cell) {
      this.metrics.placeholders++;
      context.fillStyle = theme.colours['control-fill'];
      context.fillRect(
        box.x + 8,
        box.y + (box.height - 8) / 2,
        box.width - 16,
        8,
      );
    } else {
      if (!selected) this.cells.background(context, cell);
      this.cells.focused = selected && this.isFocus(row, col);
      this.cells.content(
        context,
        cell,
        source.columns[col],
        this.text.get(cell),
      );
    }
    // Column rules only: zebra rows separate themselves (TablePlus).
    context.fillStyle = theme.colours['grid-line'];
    context.fillRect(
      box.x + box.width - 1 / this.dpr,
      box.y,
      1 / this.dpr,
      box.height,
    );
    context.restore();
  }
  private isFocus(row: number, col: number): boolean {
    const { selection } = this.props;
    return selection.row === row && selection.column === col;
  }
  private fill(
    context: CanvasRenderingContext2D,
    row: number,
    selected: boolean,
  ): void {
    const { colours } = this.props.theme,
      box = this.cells.box,
      striped = this.props.striped !== false && row % 2 === 1;
    if (selected) {
      // The selection layer repaints the cell opaquely, then tints it.
      context.fillStyle = colours.window;
      context.fillRect(box.x, box.y, box.width, box.height);
    }
    if (striped) {
      context.fillStyle = colours['row-stripe'];
      context.fillRect(box.x, box.y, box.width, box.height);
    }
    if (!selected) return;
    context.fillStyle = colours['accent-soft'];
    context.fillRect(box.x, box.y, box.width, box.height);
  }
  /** The sticky header: window colour, 11 px titles, a hairline below. */
  private header(): void {
    const context = this.contexts[2];
    if (!context) return;
    const { colours, headerFont } = this.props.theme,
      height = this.layout.headerHeight;
    context.save();
    if (headerFont) context.font = headerFont;
    for (const pinned of [false, true]) {
      context.fillStyle = colours.window;
      context.fillRect(
        0,
        0,
        pinned ? this.layout.pinnedWidth : this.view.width,
        height,
      );
      const first = pinned ? 0 : this.range.c0,
        last = pinned ? this.layout.pinned - 1 : this.range.c1;
      for (let col = first; col <= last; col++)
        this.headerCell(context, col, pinned);
    }
    context.fillStyle = colours.separator;
    context.fillRect(0, height - 1 / this.dpr, this.view.width, 1 / this.dpr);
    context.restore();
  }
  private headerCell(
    context: CanvasRenderingContext2D,
    col: number,
    pinned: boolean,
  ): void {
    const column = this.props.source.columns[col];
    if (!column) return;
    const { colours } = this.props.theme,
      height = this.layout.headerHeight;
    const x = (this.layout.offsets[col] ?? 0) - (pinned ? 0 : this.view.x);
    const arrow = column.sort ? SORT_ROOM : 0;
    context.save();
    context.beginPath();
    context.rect(x, 0, column.width - arrow, height);
    context.clip();
    context.fillStyle = column.sort ? colours.text : colours.secondary;
    context.fillText(column.title, snap(x + 8, this.dpr), height / 2);
    context.restore();
    context.fillStyle = colours['grid-line'];
    context.fillRect(x + column.width - 1 / this.dpr, 0, 1 / this.dpr, height);
    context.fillStyle = colours.secondary;
    if (column.sort)
      sortTriangle(context, {
        cx: x + column.width - SORT_ROOM / 2,
        cy: height / 2,
        up: column.sort === 'ascending',
      });
  }
}

/** Layout for a theme's row and header heights. */
function layoutFor(props: GridProps): GridLayout {
  const { rowHeight, headerHeight } = props.theme;
  return new GridLayout(
    props.source.columns,
    rowHeight,
    headerHeight ?? rowHeight,
  );
}

/** Header room reserved for the sort triangle. */
const SORT_ROOM = 18;

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
