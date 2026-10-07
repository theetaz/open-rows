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
  return {
    metrics: painter.metrics,
    viewport: painter.view,
    scrollTo: (x, y) => {
      scroll.scrollTo(x, y);
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
  scroll.onpointerdown = (event): void => {
    const bounds = scroll.getBoundingClientRect(),
      x = event.clientX - bounds.left,
      y = event.clientY - bounds.top;
    const header = painter.layout.headerHit(painter.view, x, y);
    if (header !== undefined) {
      props.onHeader?.(header);
      return;
    }
    const focus = painter.layout.hit(painter.view, x, y);
    if (!focus || focus.row >= props.source.rows) return;
    const range = {
      r0: focus.row,
      r1: focus.row,
      c0: focus.column,
      c1: focus.column,
    };
    props.selection = {
      ...focus,
      ranges:
        event.metaKey || event.ctrlKey
          ? [...props.selection.ranges, range]
          : [range],
    };
    props.onSelection(props.selection);
    accessibility.update(props.selection);
    controller.invalidate();
    host.querySelector<HTMLElement>('[role="grid"]')?.focus();
  };
}
