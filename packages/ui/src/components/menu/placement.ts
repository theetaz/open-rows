// SPDX-License-Identifier: Apache-2.0
/**
 * The one placement rule for menus, popovers and pickers: 6 px inside each
 * window edge (`--or-float-inset`), flipped to the other side of the pointer
 * or anchor when it would cross an edge, scrolling when neither side fits,
 * and never over the macOS traffic lights.
 */

/** Gap kept between a floating box and the window edges. */
const FLOAT_INSET = 6;
/** The corner the macOS traffic lights occupy, in CSS px from the top left. */
const trafficLights = { right: 84, bottom: 48 };

/** A floating box to place: its preferred top-left corner and its size. */
export type FloatingBox = {
  x: number;
  y: number;
  width: number;
  height: number;
  /** Where its bottom edge goes when it opens upward; defaults to `y`. */
  flip?: number;
  /** Where its right edge goes when it opens leftward (submenus, pointer menus). */
  flipX?: number;
  /** Open upward first (bottom-bar buttons). */
  above?: boolean;
};

/** Where a floating box goes; `maxHeight` is set when it must scroll. */
type Clamped = { left: number; top: number; maxHeight: number | null };

function horizontal(box: FloatingBox, width: number): number {
  const overflows = box.x + box.width > width - FLOAT_INSET;
  const x =
    overflows && box.flipX !== undefined && box.flipX - box.width >= FLOAT_INSET
      ? box.flipX - box.width
      : box.x;
  return Math.max(FLOAT_INSET, Math.min(x, width - FLOAT_INSET - box.width));
}

function vertical(
  box: FloatingBox,
  height: number,
): { top: number; maxHeight: number | null } {
  const flip = box.flip ?? box.y;
  const down = box.height <= height - FLOAT_INSET - box.y ? box.y : null;
  const up = box.height <= flip - FLOAT_INSET ? flip - box.height : null;
  const top = box.above ? (up ?? down) : (down ?? up);
  if (top !== null) return { top, maxHeight: null };
  const room = height - 2 * FLOAT_INSET;
  return box.height > room
    ? { top: FLOAT_INSET, maxHeight: room }
    : { top: height - FLOAT_INSET - box.height, maxHeight: null };
}

/** Clamp a floating box to the window (`view`) and away from the traffic lights. */
export function clampToWindow(
  box: FloatingBox,
  view: { width: number; height: number },
): Clamped {
  const left = horizontal(box, view.width);
  const placed = vertical(box, view.height);
  if (left >= trafficLights.right || placed.top >= trafficLights.bottom)
    return { left, ...placed };
  const top = trafficLights.bottom;
  const room = view.height - FLOAT_INSET - top;
  return { left, top, maxHeight: box.height > room ? room : null };
}

function px(value: number): string {
  return `${String(Math.round(value))}px`;
}

/**
 * Measure a fixed-position `element` and move it to where `clampToWindow`
 * puts it; `scroller` takes the height cap when the box must scroll.
 */
export function placeFloating(
  element: HTMLElement,
  at: Omit<FloatingBox, 'width' | 'height'>,
  scroller: HTMLElement = element,
): Clamped {
  scroller.style.removeProperty('max-height');
  const placed = clampToWindow(
    { ...at, width: element.offsetWidth, height: element.offsetHeight },
    { width: window.innerWidth, height: window.innerHeight },
  );
  element.style.setProperty('left', px(placed.left));
  element.style.setProperty('top', px(placed.top));
  element.style.setProperty(
    '--or-origin-x',
    px(Math.max(0, at.x - placed.left)),
  );
  element.style.setProperty(
    '--or-origin-y',
    px(Math.max(0, at.y - placed.top)),
  );
  if (placed.maxHeight !== null)
    scroller.style.setProperty('max-height', px(placed.maxHeight));
  return placed;
}

/** Where a box opens under (or, without room, over) an anchor rectangle. */
export function belowAnchor(
  anchor: Pick<DOMRect, 'left' | 'right' | 'top' | 'bottom'>,
  options: {
    width?: number | undefined;
    align?: 'start' | 'end' | undefined;
    gap?: number | undefined;
  } = {},
): Omit<FloatingBox, 'width' | 'height'> {
  const gap = options.gap ?? 4;
  const x =
    options.align === 'end' ? anchor.right - (options.width ?? 0) : anchor.left;
  return { x, y: anchor.bottom + gap, flip: anchor.top - gap };
}
