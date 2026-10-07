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
