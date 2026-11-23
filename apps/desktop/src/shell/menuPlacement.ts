// SPDX-License-Identifier: Apache-2.0
/** Horizontal extent of a box, as from `getBoundingClientRect`. */
type Extent = { left: number; right: number };

/** Gap kept between a menu and the edges of its area. */
const inset = 8;
/** Width of menus that do not match their opener. */
const preferred = 240;

/**
 * Where a sidebar menu goes: below its opener, never wider than the area it
 * belongs to (the sidebar, else the window) and shifted left to stay inside.
 * `fit` menus take the opener's width.
 */
export function menuPlacement(
  opener: Extent & { bottom: number },
  area: Extent,
  fit: boolean,
): { left: number; top: number; width: number } {
  const room = Math.max(0, area.right - area.left - 2 * inset);
  const width = Math.min(fit ? opener.right - opener.left : preferred, room);
  const left = Math.max(
    area.left + inset,
    Math.min(opener.left, area.right - inset - width),
  );
  return { left, top: opener.bottom, width };
}

/** The corner the macOS traffic lights occupy, in CSS px from the top left. */
const trafficLights = { right: 84, bottom: 48 };

/** A floating box to place: its preferred top-left corner and its size. */
type FloatingBox = {
  x: number;
  y: number;
  width: number;
  height: number;
  /** Where its bottom edge goes when it opens upward; defaults to `y`. */
  flip?: number;
  /** Open upward first (bottom-bar buttons). */
  above?: boolean;
};

/** Where a floating box goes; `maxHeight` is set when it must scroll. */
type Clamped = { left: number; top: number; maxHeight: number | null };

/**
 * The one placement rule for menus and popovers: 8 px inside each window
 * edge, upward when there is no room below (or downward when an `above`
 * menu has no room above), pinned to the bottom and scrolling when neither
 * side fits, and never over the traffic lights.
 */
export function clampToWindow(
  box: FloatingBox,
  view: { width: number; height: number },
): Clamped {
  const left = Math.max(inset, Math.min(box.x, view.width - inset - box.width));
  const placed = vertical(box, view.height);
  if (left >= trafficLights.right || placed.top >= trafficLights.bottom)
    return { left, ...placed };
  const top = trafficLights.bottom;
  const room = view.height - inset - top;
  return { left, top, maxHeight: box.height > room ? room : null };
}

function vertical(
  box: FloatingBox,
  height: number,
): { top: number; maxHeight: number | null } {
  const flip = box.flip ?? box.y;
  const down = box.height <= height - inset - box.y ? box.y : null;
  const up = box.height <= flip - inset ? flip - box.height : null;
  const top = box.above ? (up ?? down) : (down ?? up);
  if (top !== null) return { top, maxHeight: null };
  const room = height - 2 * inset;
  return box.height > room
    ? { top: inset, maxHeight: room }
    : { top: height - inset - box.height, maxHeight: null };
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
  at: Pick<FloatingBox, 'x' | 'y' | 'flip' | 'above'>,
  scroller: HTMLElement = element,
): void {
  scroller.style.removeProperty('max-height');
  const placed = clampToWindow(
    { ...at, width: element.offsetWidth, height: element.offsetHeight },
    { width: window.innerWidth, height: window.innerHeight },
  );
  element.style.setProperty('left', px(placed.left));
  element.style.setProperty('top', px(placed.top));
  if (placed.maxHeight !== null)
    scroller.style.setProperty('max-height', px(placed.maxHeight));
}
