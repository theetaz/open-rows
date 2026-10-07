// SPDX-License-Identifier: Apache-2.0
import { GridError } from './error';
import { GridLayout, snap, backingSize } from './layout';
import type { Viewport } from './layout';
import type { CellRange, GridProps, PaintMetrics } from './model';
import { TextCache } from './text';
import { paintMetrics } from './metrics';

/** Four canvas layers use persistent contexts and reusable ranges. */
export class CanvasPainter {
  readonly view: Viewport = { x: 0, y: 0, width: 0, height: 0 };
  readonly range: CellRange = { r0: 0, r1: 0, c0: 0, c1: 0 };
  readonly layout: GridLayout;
  readonly metrics: PaintMetrics = paintMetrics();
  private readonly contexts: CanvasRenderingContext2D[];
  private readonly text = new TextCache();
  private dirty = true;
  private previousX = -1;
  private previousY = -1;
  private selectionModel: GridProps['selection'] | undefined;
  private dpr = window.devicePixelRatio || 1;
  constructor(
    host: HTMLElement,
    private readonly props: GridProps,
  ) {
    this.layout = new GridLayout(props.source.columns, props.theme.rowHeight);
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
              (this.props.source.columns[col]?.width ?? 0) - 16,
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
      this.selection();
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
    context.beginPath();
    context.rect(
      left,
      this.props.theme.rowHeight,
      right - left,
      this.view.height,
    );
    context.clip();
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
  private cell(
    context: CanvasRenderingContext2D,
    row: number,
    col: number,
    pinned: boolean,
  ): void {
    const { theme, source } = this.props,
      height = theme.rowHeight;
    const x = snap(
      (this.layout.offsets[col] ?? 0) - (pinned ? 0 : this.view.x),
      this.dpr,
    );
    const y = snap((row + 1) * height - this.view.y, this.dpr),
      width = source.columns[col]?.width ?? 0;
    const cell = source.cell(row, col);
    context.save();
    context.beginPath();
    context.rect(x, y, width, height);
    context.clip();
    if (this.props.striped !== false && row % 2 === 1) {
      context.fillStyle = theme.colours['row-stripe'];
      context.fillRect(x, y, width, height);
    }
    if (!cell) {
      this.metrics.placeholders++;
      context.fillStyle = theme.colours['control-fill'];
      context.fillRect(x + 8, y + 9, width - 16, 8);
    } else {
      this.cellBackground(context, cell.state);
      context.fillStyle =
        cell.state === 'null' || cell.state === 'empty'
          ? theme.colours.tertiary
          : theme.colours.text;
      context.fillText(this.text.get(cell), x + 8, y + height / 2);
      if (cell.state === 'deleted') {
        context.fillStyle = theme.colours.destructive;
        context.fillRect(x + 8, y + height / 2, width - 16, 1 / this.dpr);
      }
    }
    context.fillStyle = theme.colours.separator;
    context.fillRect(x + width - 1 / this.dpr, y, 1 / this.dpr, height);
    context.fillRect(x, y + height - 1 / this.dpr, width, 1 / this.dpr);
    context.restore();
  }
  private cellBackground(
    context: CanvasRenderingContext2D,
    state: string,
  ): void {
    if (state !== 'changed' && state !== 'deleted') return;
    context.fillStyle =
      this.props.theme.colours[
        state === 'changed' ? 'cell-edited' : 'row-deleted'
      ];
    context.fill();
  }
  private header(): void {
    const context = this.contexts[2];
    if (!context) return;
    context.fillStyle = this.props.theme.colours['panel-solid'];
    context.fillRect(0, 0, this.view.width, this.props.theme.rowHeight);
    context.fillStyle = this.props.theme.colours.secondary;
    for (let col = this.range.c0; col <= this.range.c1; col++)
      this.headerCell(context, col, false);
    context.fillStyle = this.props.theme.colours['panel-solid'];
    context.fillRect(0, 0, this.layout.pinnedWidth, this.props.theme.rowHeight);
    context.fillStyle = this.props.theme.colours.secondary;
    for (let col = 0; col < this.layout.pinned; col++)
      this.headerCell(context, col, true);
  }
  private headerCell(
    context: CanvasRenderingContext2D,
    col: number,
    pinned: boolean,
  ): void {
    const column = this.props.source.columns[col];
    if (!column) return;
    const x = (this.layout.offsets[col] ?? 0) - (pinned ? 0 : this.view.x);
    context.save();
    context.beginPath();
    context.rect(x, 0, column.width, this.props.theme.rowHeight);
    context.clip();
    context.fillText(
      column.title,
      snap(x + 8, this.dpr),
      this.props.theme.rowHeight / 2,
    );
    context.restore();
  }
  private selection(): void {
    const context = this.contexts[3];
    if (!context) return;
    context.strokeStyle = this.props.theme.colours.focus;
    context.lineWidth = 1 / this.dpr;
    for (const range of this.props.selection.ranges) {
      this.selectionPart(context, range, true);
      this.selectionPart(context, range, false);
    }
  }
  private selectionPart(
    context: CanvasRenderingContext2D,
    range: CellRange,
    pinned: boolean,
  ): void {
    const first = pinned ? range.c0 : Math.max(range.c0, this.layout.pinned);
    const last = pinned ? Math.min(range.c1, this.layout.pinned - 1) : range.c1;
    if (first > last) return;
    const x = (this.layout.offsets[first] ?? 0) - (pinned ? 0 : this.view.x);
    const y = (range.r0 + 1) * this.props.theme.rowHeight - this.view.y;
    context.save();
    context.beginPath();
    context.rect(
      pinned ? 0 : this.layout.pinnedWidth,
      this.props.theme.rowHeight,
      pinned
        ? this.layout.pinnedWidth
        : this.view.width - this.layout.pinnedWidth,
      this.view.height,
    );
    context.clip();
    context.strokeRect(
      snap(x, this.dpr),
      snap(y, this.dpr),
      (this.layout.offsets[last + 1] ?? 0) - (this.layout.offsets[first] ?? 0),
      (range.r1 - range.r0 + 1) * this.props.theme.rowHeight,
    );
    context.restore();
  }
}
