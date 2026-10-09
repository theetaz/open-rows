// SPDX-License-Identifier: Apache-2.0
import {
  dragWidth,
  dropIndex,
  dropLine,
  fitWidth,
  resizeEdge,
} from './columns';
import type { PointerContext } from './pointer';
import { STEPPER_WIDTH } from './cellPaint';

/** Pointer travel that turns a header press into a reorder drag. */
const DRAG_THRESHOLD = 4;
/** Rows either side of the viewport sampled by a double-click auto-size. */
const SAMPLE_MARGIN = 100;

/** Pointer position relative to the scroller. */
function local(
  scroll: HTMLElement,
  event: MouseEvent,
): { x: number; y: number } {
  const bounds = scroll.getBoundingClientRect();
  return { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
}

/** Track one pointer gesture on the window until release. */
function track(
  move: (event: PointerEvent) => void,
  end: (event: PointerEvent) => void,
): void {
  const up = (event: PointerEvent): void => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('pointercancel', up);
    end(event);
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
  window.addEventListener('pointercancel', up);
}

/** A data column a header gesture may resize or move (not the row-number gutter). */
function movable(context: PointerContext, column: number): boolean {
  return column !== context.props.rowHeaderColumn;
}

/** The column edge under a header pointer, when resizing is enabled there. */
export function edgeUnder(
  context: PointerContext,
  event: MouseEvent,
): number | undefined {
  if (!context.props.onColumnResize) return undefined;
  const { x, y } = local(context.scroll, event);
  const { layout, view } = context.painter;
  const edge = resizeEdge(layout, view, x, y);
  return edge !== undefined && movable(context, edge) ? edge : undefined;
}

/** Drag a header edge: live widths while moving, `done` on release. */
export function startResize(
  context: PointerContext,
  event: PointerEvent,
  column: number,
): void {
  const start = context.props.source.columns[column]?.width ?? 0;
  const origin = event.clientX;
  let width = start;
  track(
    (move) => {
      width = dragWidth(start, move.clientX - origin);
      context.props.onColumnResize?.(column, width, false);
    },
    () => {
      context.props.onColumnResize?.(column, width, true);
    },
  );
}

/**
 * Press on a header: a drag past the threshold moves the column, a plain
 * release runs `click` (sort or column selection).
 */
export function startPress(
  context: PointerContext,
  event: PointerEvent,
  from: number,
  click: () => void,
): void {
  const origin = event.clientX;
  const canMove = !!context.props.onColumnMove && movable(context, from);
  let marker: HTMLElement | undefined;
  const target = (pointer: MouseEvent): number => {
    const { layout, view } = context.painter;
    const index = dropIndex(
      layout,
      view,
      local(context.scroll, pointer).x,
      from,
    );
    return Math.max(context.props.rowHeaderColumn === 0 ? 1 : 0, index);
  };
  track(
    (move) => {
      if (
        !canMove ||
        (!marker && Math.abs(move.clientX - origin) < DRAG_THRESHOLD)
      )
        return;
      marker ??= dropMarker(context);
      const { layout, view } = context.painter;
      marker.style.left = `${String(dropLine(layout, view, target(move), from) - 1)}px`;
    },
    (up) => {
      marker?.remove();
      // A cancelled pointer (e.g. the system took it) neither sorts nor moves.
      if (up.type === 'pointercancel') return;
      if (!marker) {
        click();
        return;
      }
      const to = target(up);
      if (to !== from) context.props.onColumnMove?.(from, to);
    },
  );
}

/** A thin accent bar where the dragged header will land. */
function dropMarker(context: PointerContext): HTMLElement {
  const marker = document.createElement('div');
  marker.dataset.dropMarker = '';
  marker.style.cssText = `position:absolute;top:0;bottom:0;width:2px;pointer-events:none;background:${context.props.theme.colours.accent};`;
  context.scroll.parentElement?.append(marker);
  return marker;
}

/** Width that fits a column's title and its loaded values near the viewport. */
export function autoWidth(context: PointerContext, column: number): number {
  const { painter, props } = context;
  const info = props.source.columns[column];
  const measure = painter.measure;
  const { r0, r1 } = painter.range;
  const first = Math.max(0, r0 - SAMPLE_MARGIN),
    last = Math.min(props.source.rows - 1, r1 + SAMPLE_MARGIN);
  const widths = [measure(info?.title ?? '')];
  for (let row = first; row <= last; row++) {
    const cell = props.source.cell(row, column);
    if (cell) widths.push(measure(cell.text));
  }
  return fitWidth(widths, info?.kind ? STEPPER_WIDTH : 0);
}
