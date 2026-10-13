// SPDX-License-Identifier: Apache-2.0
import type { CanvasController } from './canvasController';
import type { CanvasPainter } from './painter';
import type { shadowGrid } from './accessibility';
import type { GridProps } from './model';
import { STEPPER_WIDTH } from './cellPaint';
import { autoWidth, edgeUnder, startPress, startResize } from './headerDrag';

export interface PointerContext {
  scroll: HTMLElement;
  painter: CanvasPainter;
  props: GridProps;
  controller: CanvasController;
  accessibility: ReturnType<typeof shadowGrid>;
}
export function pointerFocus({
  scroll,
  painter,
  props,
  controller,
  accessibility,
}: PointerContext): void {
  const context = { scroll, painter, props, controller, accessibility };
  scroll.ondblclick = (event): void => {
    const edge = edgeUnder(context, event);
    if (edge !== undefined) {
      props.onColumnResize?.(edge, autoWidth(context, edge), true);
      return;
    }
    const bounds = scroll.getBoundingClientRect();
    const cell = painter.layout.hit(
      painter.view,
      event.clientX - bounds.left,
      event.clientY - bounds.top,
    );
    if (
      cell &&
      cell.row < props.source.rows &&
      cell.column !== props.rowHeaderColumn
    )
      props.onEdit?.(cell);
  };
  scroll.onpointerdown = (event): void => {
    if (event.button !== 0) return;
    const bounds = scroll.getBoundingClientRect(),
      x = event.clientX - bounds.left,
      y = event.clientY - bounds.top;
    const edge = edgeUnder(context, event);
    if (edge !== undefined) {
      event.preventDefault();
      startResize(context, event, edge);
      return;
    }
    const header = painter.layout.headerHit(painter.view, x, y);
    if (header !== undefined) {
      const shift = event.shiftKey;
      startPress(context, event, header, () => {
        headerClick(context, header, shift);
      });
      return;
    }
    const focus = painter.layout.hit(painter.view, x, y);
    if (!focus || focus.row >= props.source.rows) return;
    const onRowNumber = focus.column === props.rowHeaderColumn;
    selectPointer(props, focus, event);
    props.onSelection(props.selection);
    accessibility.update(props.selection);
    controller.invalidate();
    accessibility.focus();
    if (event.shiftKey || event.metaKey) return;
    const glyph = glyphHit(painter, focus, x);
    if (glyph) openOnRelease(props, glyph, focus);
    else startCellDrag(context, event, onRowNumber);
  };
}
/**
 * Run a glyph's action when the press ends. The press itself still moves
 * focus to the scroller (cancelling pointerdown does not stop that), which
 * would close a menu opened on pointerdown at once.
 */
function openOnRelease(
  props: GridProps,
  glyph: 'stepper' | 'link',
  cell: { row: number; column: number },
): void {
  const run = (): void => {
    window.removeEventListener('pointercancel', drop);
    if (glyph === 'stepper') (props.onStepper ?? props.onEdit)?.(cell);
    else props.onLink?.(cell);
  };
  const drop = (): void => {
    window.removeEventListener('pointerup', run);
  };
  window.addEventListener('pointerup', run, { once: true });
  window.addEventListener('pointercancel', drop, { once: true });
}
/**
 * Dragging from a cell extends the selection to the cell under the pointer,
 * across rows and columns; from the row numbers it selects whole rows. The
 * grid scrolls when the pointer leaves its edges.
 */
function startCellDrag(
  context: PointerContext,
  down: PointerEvent,
  rows: boolean,
): void {
  const { scroll, painter, props } = context;
  const anchor = props.selection.anchor ?? props.selection;
  let last = `${String(anchor.row)}:${String(anchor.column)}`;
  const move = (event: PointerEvent): void => {
    const bounds = scroll.getBoundingClientRect();
    edgeScroll(scroll, bounds, event);
    const x = Math.min(
        Math.max(event.clientX - bounds.left, 0),
        bounds.width - 1,
      ),
      y = Math.min(
        Math.max(event.clientY - bounds.top, painter.layout.rowHeight),
        bounds.height - 1,
      );
    const cell = painter.layout.hit(painter.view, x, y);
    if (!cell || cell.row >= props.source.rows) return;
    const column = Math.max(
      cell.column,
      props.rowHeaderColumn === undefined ? 0 : props.rowHeaderColumn + 1,
    );
    const key = `${String(cell.row)}:${String(column)}`;
    if (key === last) return;
    last = key;
    dragTo(context, anchor, { row: cell.row, column }, rows);
  };
  const up = (): void => {
    scroll.removeEventListener('pointermove', move);
    scroll.removeEventListener('pointerup', up);
    scroll.removeEventListener('pointercancel', up);
  };
  try {
    scroll.setPointerCapture(down.pointerId);
  } catch {
    // Synthetic or finished pointers cannot be captured; moves inside the grid still extend.
  }
  scroll.addEventListener('pointermove', move);
  scroll.addEventListener('pointerup', up);
  scroll.addEventListener('pointercancel', up);
}
/** The rectangle from the anchor to `to`, or whole rows when the drag began on a row number. */
function dragTo(
  { props, accessibility, controller }: PointerContext,
  anchor: { row: number; column: number },
  to: { row: number; column: number },
  rows: boolean,
): void {
  const first =
    props.rowHeaderColumn === undefined ? 0 : props.rowHeaderColumn + 1;
  props.selection = {
    ...props.selection,
    ranges: [
      {
        r0: Math.min(anchor.row, to.row),
        r1: Math.max(anchor.row, to.row),
        c0: rows ? first : Math.min(anchor.column, to.column),
        c1: rows
          ? props.source.columns.length - 1
          : Math.max(anchor.column, to.column),
      },
    ],
  };
  props.onSelection(props.selection);
  accessibility.update(props.selection);
  controller.invalidate();
}
/** Scroll a step toward a pointer that has left the grid's edges during a drag. */
function edgeScroll(
  scroll: HTMLElement,
  bounds: DOMRect,
  event: PointerEvent,
): void {
  const step = 24;
  if (event.clientY > bounds.bottom - 8) scroll.scrollTop += step;
  else if (event.clientY < bounds.top + 8) scroll.scrollTop -= step;
  if (event.clientX > bounds.right - 8) scroll.scrollLeft += step;
  else if (event.clientX < bounds.left + 8) scroll.scrollLeft -= step;
}
/** The row-number header fits all columns (or selects them without a host); Shift-click selects a column; a click sorts. */
function headerClick(
  { props, accessibility, controller }: PointerContext,
  header: number,
  shift: boolean,
): void {
  if (!shift && header === props.rowHeaderColumn && props.onFitColumns) {
    props.onFitColumns();
    return;
  }
  if ((shift || header === props.rowHeaderColumn) && props.source.rows > 0) {
    props.selection = {
      row: 0,
      column: header === props.rowHeaderColumn ? header + 1 : header,
      ranges: [{ r0: 0, r1: props.source.rows - 1, c0: header, c1: header }],
    };
    props.onSelection(props.selection);
    accessibility.update(props.selection);
    controller.invalidate();
  } else if (header !== props.rowHeaderColumn) props.onHeader?.(header);
}
/** Show the resize cursor over header edges. */
export function hoverEdges(context: PointerContext): void {
  context.scroll.onpointermove = (event): void => {
    const cursor = edgeUnder(context, event) === undefined ? '' : 'col-resize';
    if (context.scroll.style.cursor !== cursor)
      context.scroll.style.cursor = cursor;
  };
}
/**
 * The end-of-cell glyph under a click: a boolean or enum cell's ⌃⌄ stepper
 * opens its value menu; a foreign key's → arrow (left of any stepper) opens
 * the referenced row.
 */
function glyphHit(
  painter: CanvasPainter,
  cell: { row: number; column: number },
  x: number,
): 'stepper' | 'link' | undefined {
  const column = painter.props.source.columns[cell.column];
  if (!column?.kind && !column?.link) return undefined;
  const pinned = cell.column < painter.layout.pinned;
  const right =
    (painter.layout.offsets[cell.column + 1] ?? 0) -
    (pinned ? 0 : painter.view.x);
  const stepper = column.kind ? STEPPER_WIDTH : 0;
  if (stepper && x >= right - STEPPER_WIDTH - 4 && x <= right) return 'stepper';
  const end = right - stepper;
  const onArrow = column.link && x >= end - STEPPER_WIDTH - 4 && x <= end;
  return onArrow && linked(painter, cell) ? 'link' : undefined;
}
/** A key cell with a value (NULL and empty keys reference nothing). */
function linked(
  painter: CanvasPainter,
  cell: { row: number; column: number },
): boolean {
  const value = painter.props.source.cell(cell.row, cell.column);
  return (
    value !== undefined && value.state !== 'null' && value.state !== 'empty'
  );
}
export function contextFocus({
  scroll,
  painter,
  props,
  controller,
  accessibility,
}: PointerContext): void {
  scroll.oncontextmenu = (event): void => {
    event.preventDefault();
    const bounds = scroll.getBoundingClientRect();
    const target = contextTarget(
      painter,
      props,
      event.clientX - bounds.left,
      event.clientY - bounds.top,
    );
    if (!target) return;
    const { header, focus, rowHeader } = target;
    // A right-click inside the selection keeps it, so its actions apply to every selected cell.
    if (header === undefined && !rowHeader && inSelection(props, focus))
      props.selection = { ...props.selection, ...focus };
    else selectPointer(props, { ...focus }, event);
    if (header !== undefined)
      props.selection.ranges = [
        {
          r0: 0,
          r1: Math.max(0, props.source.rows - 1),
          c0: header,
          c1: header,
        },
      ];
    props.onSelection(props.selection);
    accessibility.update(props.selection);
    controller.invalidate();
    accessibility.focus();
    props.onContextMenu?.({
      ...focus,
      column: rowHeader ? focus.column + 1 : focus.column,
      kind: header !== undefined ? 'column' : rowHeader ? 'row' : 'cell',
      x: event.clientX,
      y: event.clientY,
    });
  };
}

type ContextTarget = {
  header: number | undefined;
  focus: { row: number; column: number };
  rowHeader: boolean;
};

/** The header or cell under a right-click, or `undefined` for blank space and the row-number header. */
function contextTarget(
  painter: CanvasPainter,
  props: GridProps,
  x: number,
  y: number,
): ContextTarget | undefined {
  const header = painter.layout.headerHit(painter.view, x, y);
  const cell = painter.layout.hit(painter.view, x, y);
  if (header === undefined && (!cell || cell.row >= props.source.rows))
    return undefined;
  // The row-number header has no actions; without a row-number column every header counts.
  if (header !== undefined && header === props.rowHeaderColumn)
    return undefined;
  const focus = header !== undefined ? { row: 0, column: header } : cell;
  if (!focus) return undefined;
  return { header, focus, rowHeader: focus.column === props.rowHeaderColumn };
}

function inSelection(
  props: GridProps,
  cell: { row: number; column: number },
): boolean {
  return props.selection.ranges.some(
    (range) =>
      cell.row >= range.r0 &&
      cell.row <= range.r1 &&
      cell.column >= range.c0 &&
      cell.column <= range.c1,
  );
}

function selectPointer(
  props: GridProps,
  focus: { row: number; column: number },
  event: MouseEvent,
): void {
  const anchor = event.shiftKey
    ? (props.selection.anchor ?? props.selection)
    : focus;
  const rowHeader = focus.column === props.rowHeaderColumn;
  const range = {
    r0: event.shiftKey ? Math.min(anchor.row, focus.row) : focus.row,
    r1: event.shiftKey ? Math.max(anchor.row, focus.row) : focus.row,
    c0: rowHeader
      ? focus.column + 1
      : event.shiftKey
        ? Math.min(anchor.column, focus.column)
        : focus.column,
    c1: rowHeader
      ? props.source.columns.length - 1
      : event.shiftKey
        ? Math.max(anchor.column, focus.column)
        : focus.column,
  };
  if (rowHeader) focus.column++;
  props.selection = {
    ...focus,
    anchor: { row: anchor.row, column: anchor.column },
    ranges:
      event.metaKey || event.ctrlKey
        ? [...props.selection.ranges, range]
        : [range],
  };
}
