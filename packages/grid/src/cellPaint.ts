// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell, GridColumn, GridTheme } from './model';
import { booleanText } from '@openrows/ui/format';
import { statusTone } from './statusTone';
import type { StatusTone } from './statusTone';

/** The status colours of each tone. */
const TONE_COLOUR = {
  ok: 'success-fg',
  bad: 'danger-fg',
  run: 'info-fg',
  wait: 'warning-fg',
} as const;

/** Horizontal text inset and the width reserved for a stepper glyph. */
const CELL_PADDING = 8;
export const STEPPER_WIDTH = 16;
/** Side of the staged-cell corner mark. */
const MARK = 6;

/** Width of the glyphs at a cell's end: a stepper and a link arrow. */
export function accessoryWidth(column: GridColumn | undefined): number {
  return (
    (column?.kind ? STEPPER_WIDTH : 0) + (column?.link ? STEPPER_WIDTH : 0)
  );
}

/** Width available to a cell's text, excluding padding, stepper and arrow. */
export function textWidth(column: GridColumn | undefined): number {
  if (!column) return 0;
  return column.width - CELL_PADDING * 2 - accessoryWidth(column);
}

/**
 * Whether a cell paints its stepper: boolean and enum cells always do; a
 * date or time cell only while focused, so a column of timestamps stays quiet.
 */
export function showsStepper(
  column: GridColumn | undefined,
  focused: boolean,
): boolean {
  return column?.kind !== undefined && (column.kind !== 'temporal' || focused);
}

/** Muted markers: NULL, EMPTY and DEFAULT, distinct from real values. */
export function isMuted(cell: DisplayCell): boolean {
  return (
    cell.state === 'null' || cell.state === 'empty' || cell.marker !== undefined
  );
}

/** Paint one cell's contents into a reusable box; never allocates. */
export class CellPainter {
  readonly box = { x: 0, y: 0, width: 0, height: 0 };
  dpr = 1;
  /** The cell being painted is the focused one (set by the painter per cell). */
  focused = false;
  /** The cell is a row number: tertiary, or accent on a selected row. */
  gutter: 'plain' | 'selected' | null = null;
  constructor(private colours: GridTheme['colours']) {}
  setColours(colours: GridTheme['colours']): void {
    this.colours = colours;
  }
  /** Staged-state fill inside the current clip. */
  background(context: CanvasRenderingContext2D, cell: DisplayCell): void {
    const fill =
      cell.state === 'changed'
        ? this.colours['cell-edited']
        : cell.state === 'deleted'
          ? this.colours['row-deleted']
          : cell.state === 'inserted'
            ? this.colours['row-inserted']
            : undefined;
    if (!fill) return;
    context.fillStyle = fill;
    context.fillRect(this.box.x, this.box.y, this.box.width, this.box.height);
  }
  /** Text, stepper, strike-through and staged marker. */
  content(
    context: CanvasRenderingContext2D,
    cell: DisplayCell,
    column: GridColumn | undefined,
    text: string,
  ): void {
    const { x, y, width, height } = this.box;
    const muted = isMuted(cell);
    context.fillStyle = this.ink(muted);
    const tone = muted || column?.kind !== 'enum' ? null : statusTone(text);
    if (tone) this.status(context, tone, text);
    else this.text(context, column, muted ? text : this.display(column, text));
    if (showsStepper(column, this.focused))
      this.stepper(context, x + width - STEPPER_WIDTH / 2 - 2);
    if (column?.link && cell.state !== 'null' && cell.state !== 'empty')
      this.arrow(
        context,
        x + width - (column.kind ? STEPPER_WIDTH : 0) - STEPPER_WIDTH / 2 - 2,
      );
    if (cell.state === 'deleted') {
      context.fillStyle = this.colours.destructive;
      context.fillRect(
        x + CELL_PADDING,
        y + height / 2,
        width - CELL_PADDING * 2,
        1 / this.dpr,
      );
    }
    if (cell.state === 'changed') this.marker(context);
  }
  private ink(muted: boolean): string {
    if (this.gutter === 'selected') return this.colours.accent;
    if (this.gutter) return this.colours.tertiary;
    return muted ? this.colours.placeholder : this.colours.text;
  }
  /** A status-like enum value: a 6 px dot and the value in its tone. */
  private status(
    context: CanvasRenderingContext2D,
    tone: StatusTone,
    text: string,
  ): void {
    const { x, y, height } = this.box;
    context.fillStyle = this.colours[TONE_COLOUR[tone]];
    context.beginPath();
    context.arc(x + CELL_PADDING + 3, y + height / 2, 3, 0, Math.PI * 2);
    context.fill();
    context.textAlign = 'left';
    context.fillText(text, x + CELL_PADDING + 12, y + height / 2);
  }
  private display(column: GridColumn | undefined, text: string): string {
    return column?.kind === 'boolean' ? booleanText(text) : text;
  }
  private text(
    context: CanvasRenderingContext2D,
    column: GridColumn | undefined,
    text: string,
  ): void {
    const { x, y, width, height } = this.box;
    const stepper = accessoryWidth(column);
    if (column?.kind === 'boolean') {
      context.textAlign = 'center';
      context.fillText(text, x + (width - stepper) / 2, y + height / 2);
    } else if (column?.align === 'end') {
      context.textAlign = 'right';
      context.fillText(
        text,
        x + width - CELL_PADDING - stepper,
        y + height / 2,
      );
    } else {
      context.textAlign = 'left';
      context.fillText(text, x + CELL_PADDING, y + height / 2);
    }
  }
  /** A ⌃⌄ pair, like a native pop-up button, centred on `cx`. */
  private stepper(context: CanvasRenderingContext2D, cx: number): void {
    const cy = this.box.y + this.box.height / 2;
    context.strokeStyle = this.colours.secondary;
    context.lineWidth = 1.25;
    context.beginPath();
    context.moveTo(cx - 3, cy - 1.5);
    context.lineTo(cx, cy - 4.5);
    context.lineTo(cx + 3, cy - 1.5);
    context.moveTo(cx - 3, cy + 1.5);
    context.lineTo(cx, cy + 4.5);
    context.lineTo(cx + 3, cy + 1.5);
    context.stroke();
  }
  /** A → in a small circle, like TablePlus' foreign-key jump, centred on `cx`. */
  private arrow(context: CanvasRenderingContext2D, cx: number): void {
    const cy = this.box.y + this.box.height / 2;
    context.strokeStyle = this.colours.secondary;
    context.lineWidth = 1.25;
    context.beginPath();
    context.arc(cx, cy, 5.5, 0, Math.PI * 2);
    context.moveTo(cx - 2.5, cy);
    context.lineTo(cx + 2.5, cy);
    context.moveTo(cx, cy - 2.5);
    context.lineTo(cx + 2.5, cy);
    context.lineTo(cx, cy + 2.5);
    context.stroke();
  }
  /** A staged cell: the edit tint (drawn first) plus a small corner mark. */
  private marker(context: CanvasRenderingContext2D): void {
    const { x, y, width } = this.box,
      right = x + width;
    context.fillStyle = this.colours['warning-fg'];
    context.beginPath();
    context.moveTo(right - MARK, y);
    context.lineTo(right, y);
    context.lineTo(right, y + MARK);
    context.closePath();
    context.fill();
  }
}
