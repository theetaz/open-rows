// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { Point } from './geometry';
import { centreOn, fitView, zoomAt } from './viewport';
import type { Size, View } from './viewport';

/** Pointer travel (px) after which a press pans instead of clicking. */
const DRAG_SLOP = 4;
/** jsdom and hidden sheets measure 0 × 0. */
const FALLBACK: Size = { width: 800, height: 600 };
const START: View = { x: 0, y: 0, scale: 1 };

function measure(element: Element | null): Size {
  const rect = element?.getBoundingClientRect();
  return rect?.width && rect.height
    ? { width: rect.width, height: rect.height }
    : FALLBACK;
}

/** Trackpad scroll pans; pinch (or ⌘/Ctrl + wheel) zooms at the pointer. */
function useWheel(
  element: SVGSVGElement | HTMLCanvasElement | null,
  setView: (update: (view: View) => View) => void,
): void {
  useEffect(() => {
    if (!element) return;
    const onWheel = (event: Event): void => {
      if (!(event instanceof WheelEvent)) return;
      event.preventDefault();
      if (event.ctrlKey || event.metaKey) {
        const rect = element.getBoundingClientRect();
        const at = {
          x: event.clientX - rect.left,
          y: event.clientY - rect.top,
        };
        setView((view) => zoomAt(view, Math.exp(-event.deltaY * 0.01), at));
      } else
        setView((view) => ({
          ...view,
          x: view.x - event.deltaX,
          y: view.y - event.deltaY,
        }));
    };
    element.addEventListener('wheel', onWheel, { passive: false });
    return () => {
      element.removeEventListener('wheel', onWheel);
    };
  }, [element, setView]);
}

/**
 * Pan and zoom of the diagram canvas: wheel and drag gestures, zoom
 * buttons, fit-to-view and centring on a table. Until the user moves it,
 * the view is `initial` for the canvas size.
 */
export function usePanZoom(initial: (viewport: Size) => View): {
  view: View;
  /** Ref callback for the canvas `<svg>`. */
  attach: (element: SVGSVGElement | HTMLCanvasElement | null) => void;
  /** The last press panned, so its click must not open a table. */
  panned: boolean;
  onPointerDown: (event: React.PointerEvent) => void;
  zoomBy: (factor: number) => void;
  fit: (content: Size) => void;
  centre: (point: Point) => void;
} {
  const [element, setElement] = useState<
    SVGSVGElement | HTMLCanvasElement | null
  >(null);
  const [moved, setMoved] = useState<View | null>(null);
  const dragCleanup = useRef<(() => void) | null>(null);
  useEffect(
    () => () => {
      dragCleanup.current?.();
    },
    [],
  );
  const [panned, setPanned] = useState(false);
  const placed = (): View => initial(measure(element));
  const view = moved ?? (element ? placed() : START);
  const update = (change: (view: View) => View): void => {
    setMoved((previous) => change(previous ?? placed()));
  };
  useWheel(element, update);
  const onPointerDown = (event: React.PointerEvent): void => {
    if (event.button !== 0) return;
    dragCleanup.current?.();
    const start = { x: event.clientX, y: event.clientY };
    const from = view;
    let dragging = false;
    setPanned(false);
    const move = (pointer: PointerEvent): void => {
      const dx = pointer.clientX - start.x;
      const dy = pointer.clientY - start.y;
      if (!dragging && Math.hypot(dx, dy) <= DRAG_SLOP) return;
      dragging = true;
      setPanned(true);
      setMoved({ ...from, x: from.x + dx, y: from.y + dy });
    };
    const up = (): void => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', up);
      dragCleanup.current = null;
    };
    dragCleanup.current = up;
    window.addEventListener('pointercancel', up);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  return {
    view,
    attach: setElement,
    panned,
    onPointerDown,
    zoomBy: (factor) => {
      const size = measure(element);
      setMoved(zoomAt(view, factor, { x: size.width / 2, y: size.height / 2 }));
    },
    fit: (content) => {
      setMoved(fitView(content, measure(element)));
    },
    centre: (point) => {
      setMoved(centreOn(view, point, measure(element)));
    },
  };
}
