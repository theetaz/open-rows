// SPDX-License-Identifier: Apache-2.0
import {
  contentHeight,
  spacerHeight,
  logicalOffset,
  nativeOffset,
} from './scroll';
import { CanvasController } from './canvasController';
import { CanvasPainter } from './painter';
import { shadowGrid } from './accessibility';
import { contextFocus, hoverEdges, pointerFocus } from './pointer';
import { autoWidth } from './headerDrag';
import type { PointerContext } from './pointer';
import type { CellRect, GridHandle, GridProps } from './model';

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
  spacerWidth(scroll, painter);
  const fit = (): void => {
    spacer.style.height = `${String(spacerHeight(contentHeight(props)))}px`;
  };
  fit();
  const controller = new CanvasController(scroll, painter, props);
  const accessibility = shadowGrid(host, props, (selection) => {
    reveal(painter, scroll, selection);
  });
  scroll.tabIndex = -1;
  // A click would otherwise leave focus on the scroller, which ignores arrow keys.
  scroll.onfocus = accessibility.focus;
  host.addEventListener('keydown', controller.invalidate);
  pointerFocus({ scroll, painter, props, controller, accessibility });
  contextFocus({ scroll, painter, props, controller, accessibility });
  hoverEdges({ scroll, painter, props, controller, accessibility });
  const scrolled = (): void => {
    props.onScroll?.();
  };
  scroll.addEventListener('scroll', scrolled, { passive: true });
  const unsubscribe = rowCountSync(props, () => {
    fit();
    controller.refetch();
  });
  const parts = { scroll, painter, props, controller, accessibility };
  return {
    ...gridHandle(parts, fit),
    destroy: () => {
      unsubscribe();
      scroll.removeEventListener('scroll', scrolled);
      controller.destroy();
      host.removeEventListener('keydown', controller.invalidate);
      accessibility.destroy();
      scroll.remove();
      layers.remove();
    },
  };
}

/** Imperative operations for the host app: theme, geometry, focus and selection. */
function gridHandle(
  { scroll, painter, props, controller, accessibility }: PointerContext,
  fit: () => void,
): Omit<GridHandle, 'destroy'> {
  return {
    metrics: painter.metrics,
    viewport: painter.view,
    scrollTo: (x, y) => {
      scroll.scrollTo(
        x,
        nativeOffset(y, scroll.clientHeight, contentHeight(props)),
      );
    },
    setTheme: (theme) => {
      painter.setTheme(theme);
      fit();
      controller.update();
      controller.invalidate();
    },
    refresh: () => {
      painter.refresh();
      controller.invalidate();
    },
    relayout: () => {
      painter.relayout();
      spacerWidth(scroll, painter);
      controller.refetch();
    },
    fitWidth: (column) =>
      autoWidth({ scroll, painter, props, controller, accessibility }, column),
    cellRect: (row, column) => cellRect(painter, scroll, row, column),
    select: (row, column) => {
      props.selection = {
        row,
        column,
        anchor: { row, column },
        ranges: [{ r0: row, r1: row, c0: column, c1: column }],
      };
      reveal(painter, scroll, props.selection);
      accessibility.update(props.selection);
      props.onSelection(props.selection);
      controller.invalidate();
    },
    focus: accessibility.focus,
    announce: accessibility.announce,
  };
}

/** The scroll spacer spans every column so native scrolling reaches the last one. */
function spacerWidth(scroll: HTMLElement, painter: CanvasPainter): void {
  const spacer = scroll.firstElementChild;
  if (spacer instanceof HTMLElement)
    spacer.style.width = `${String(painter.layout.width)}px`;
}

/** Staged inserts change the row count without remounting the grid. */
function rowCountSync(props: GridProps, changed: () => void): () => void {
  let rows = props.source.rows;
  return props.source.subscribe(() => {
    if (props.source.rows === rows) return;
    rows = props.source.rows;
    changed();
  });
}

/** Read the live scroll offsets, which scripted reveals update synchronously. */
function cellRect(
  painter: CanvasPainter,
  scroll: HTMLElement,
  row: number,
  column: number,
): CellRect | undefined {
  const width = painter.props.source.columns[column]?.width;
  if (width === undefined || row < 0) return undefined;
  const pinned = column < painter.layout.pinned,
    height = painter.props.theme.rowHeight;
  return {
    x: (painter.layout.offsets[column] ?? 0) - (pinned ? 0 : scroll.scrollLeft),
    y:
      (row + 1) * height -
      logicalOffset(
        scroll.scrollTop,
        scroll.clientHeight,
        contentHeight(painter.props),
      ),
    width,
    height,
  };
}

/** Scroll the least distance that shows a cell below the header and right of pins. */
function reveal(
  painter: CanvasPainter,
  scroll: HTMLElement,
  { row, column }: { row: number; column: number },
): void {
  const height = painter.props.theme.rowHeight,
    top = (row + 1) * height,
    content = contentHeight(painter.props),
    current = logicalOffset(scroll.scrollTop, scroll.clientHeight, content);
  if (top - height < current)
    scroll.scrollTop = nativeOffset(top - height, scroll.clientHeight, content);
  else if (top + height > current + scroll.clientHeight)
    scroll.scrollTop = nativeOffset(
      top + height - scroll.clientHeight,
      scroll.clientHeight,
      content,
    );
  const { offsets, pinned, pinnedWidth } = painter.layout;
  if (column < pinned) return;
  const left = (offsets[column] ?? 0) - pinnedWidth,
    right = offsets[column + 1] ?? 0;
  if (left < scroll.scrollLeft) scroll.scrollLeft = left;
  else if (right > scroll.scrollLeft + scroll.clientWidth)
    scroll.scrollLeft = right - scroll.clientWidth;
}
