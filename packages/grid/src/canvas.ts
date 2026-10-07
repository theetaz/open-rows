// SPDX-License-Identifier: Apache-2.0
import { CanvasController } from './canvasController';
import { CanvasPainter } from './painter';
import { shadowGrid } from './accessibility';
import type { GridHandle, GridProps } from './model';

/** Native momentum scrolling with an independently positioned canvas viewport. */
export function mountGrid(host: HTMLElement, props: GridProps): GridHandle {
  const scroll = document.createElement('div'),
    spacer = document.createElement('div'),
    layers = document.createElement('div');
  host.style.position = 'relative';
  scroll.style.cssText = 'position:absolute;inset:0;overflow:auto;';
  layers.style.cssText = 'position:absolute;inset:0;pointer-events:none;';
  scroll.append(spacer);
  host.append(scroll, layers);
  const painter = new CanvasPainter(layers, props);
  spacer.style.width = `${String(painter.layout.width)}px`;
  spacer.style.height = `${String((props.source.rows + 1) * props.theme.rowHeight)}px`;
  const controller = new CanvasController(scroll, painter, props);
  const accessibility = shadowGrid(host, props);
  scroll.tabIndex = -1;
  host.addEventListener('keydown', controller.invalidate);
  pointerFocus({ host, scroll, painter, props, controller, accessibility });
  contextFocus({ host, scroll, painter, props, controller, accessibility });
  return {
    metrics: painter.metrics,
    viewport: painter.view,
    scrollTo: (x, y) => {
      scroll.scrollTo(x, y);
    },
    setTheme: (theme) => {
      painter.setTheme(theme);
      spacer.style.height = `${String((props.source.rows + 1) * theme.rowHeight)}px`;
      controller.update();
      controller.invalidate();
    },
    destroy: () => {
      controller.destroy();
      host.removeEventListener('keydown', controller.invalidate);
      accessibility.destroy();
      scroll.remove();
      layers.remove();
    },
  };
}

interface PointerContext {
  host: HTMLElement;
  scroll: HTMLElement;
  painter: CanvasPainter;
  props: GridProps;
  controller: CanvasController;
  accessibility: ReturnType<typeof shadowGrid>;
}
function pointerFocus({
  host,
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
    host.querySelector<HTMLElement>('[role="grid"]')?.focus();
  };
}
function contextFocus({
  host,
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
    host.querySelector<HTMLElement>('[role="grid"]')?.focus();
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
