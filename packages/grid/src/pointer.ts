// SPDX-License-Identifier: Apache-2.0
import type { CanvasController } from './canvasController';
import type { CanvasPainter } from './painter';
import type { shadowGrid } from './accessibility';
import type { GridProps } from './model';
import { STEPPER_WIDTH } from './cellPaint';

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
  scroll.ondblclick = (event): void => {
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
    const header = painter.layout.headerHit(painter.view, x, y);
    if (header !== undefined) {
      if (
        (event.shiftKey || header === props.rowHeaderColumn) &&
        props.source.rows > 0
      ) {
        props.selection = {
          row: 0,
          column: header === props.rowHeaderColumn ? header + 1 : header,
          ranges: [
            { r0: 0, r1: props.source.rows - 1, c0: header, c1: header },
          ],
        };
        props.onSelection(props.selection);
        accessibility.update(props.selection);
        controller.invalidate();
      } else if (header !== props.rowHeaderColumn) props.onHeader?.(header);
      return;
    }
    const focus = painter.layout.hit(painter.view, x, y);
    if (!focus || focus.row >= props.source.rows) return;
    selectPointer(props, focus, event);
    props.onSelection(props.selection);
    accessibility.update(props.selection);
    controller.invalidate();
    accessibility.focus();
    if (!event.shiftKey && !event.metaKey && stepperHit(painter, focus, x))
      props.onEdit?.(focus);
  };
}
/** A click on a boolean or enum cell's ⌃⌄ glyph opens its value menu. */
function stepperHit(
  painter: CanvasPainter,
  cell: { row: number; column: number },
  x: number,
): boolean {
  if (!painter.props.source.columns[cell.column]?.kind) return false;
  const pinned = cell.column < painter.layout.pinned;
  const right =
    (painter.layout.offsets[cell.column + 1] ?? 0) -
    (pinned ? 0 : painter.view.x);
  return x >= right - STEPPER_WIDTH - 4 && x <= right;
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
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    const header = painter.layout.headerHit(painter.view, x, y);
    const cell = painter.layout.hit(painter.view, x, y);
    if (header === undefined && (!cell || cell.row >= props.source.rows))
      return;
    if (header === props.rowHeaderColumn) return;
    const focus = header !== undefined ? { row: 0, column: header } : cell;
    if (!focus) return;
    const rowHeader = focus.column === props.rowHeaderColumn;
    selectPointer(props, { ...focus }, event);
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
