// SPDX-License-Identifier: Apache-2.0
import type { GridLayout, Viewport } from './layout';

/** Narrowest width a header drag can leave a column. */
export const MIN_COLUMN_WIDTH = 40;
/** Widest width a double-click auto-size picks; longer values stay truncated. */
export const MAX_AUTO_WIDTH = 480;
/** Pointer distance either side of a header edge that grabs it. */
const EDGE_SLOP = 4;
/** Horizontal text inset on both sides of a cell, as painted. */
const TEXT_INSET = 16;

/** Where a column's right edge is drawn now, in host CSS pixels. */
function screenRight(
  layout: GridLayout,
  view: Viewport,
  column: number,
): number {
  const right = layout.offsets[column + 1] ?? 0;
  return column < layout.pinned ? right : right - view.x;
}

/**
 * The column whose right edge lies under `x` in the header row, if any.
 * Scrolled columns hidden behind the pinned prefix never match.
 */
export function resizeEdge(
  layout: GridLayout,
  view: Viewport,
  x: number,
  y: number,
): number | undefined {
  if (y < 0 || y >= layout.rowHeight) return undefined;
  for (const probe of [x - EDGE_SLOP, x + EDGE_SLOP]) {
    const column = layout.columnAt(
      probe < layout.pinnedWidth ? probe : probe + view.x,
    );
    const right = screenRight(layout, view, column);
    const hidden = column >= layout.pinned && right <= layout.pinnedWidth;
    if (!hidden && Math.abs(right - x) <= EDGE_SLOP) return column;
  }
  return undefined;
}

/**
 * The index a dragged header lands on when dropped at `x`: the column under
 * the pointer, or the slot after it past its midpoint. `from` is the dragged
 * column; the result is its index after the move.
 */
export function dropIndex(
  layout: GridLayout,
  view: Viewport,
  x: number,
  from: number,
): number {
  const content = x < layout.pinnedWidth ? x : x + view.x;
  const column = layout.columnAt(Math.max(0, content));
  const left = layout.offsets[column] ?? 0,
    right = layout.offsets[column + 1] ?? left;
  const slot = content > (left + right) / 2 ? column + 1 : column;
  return slot > from ? slot - 1 : slot;
}

/** Host x of the boundary a header dropped at `index` would land on. */
export function dropLine(
  layout: GridLayout,
  view: Viewport,
  index: number,
  from: number,
): number {
  const slot = index > from ? index + 1 : index;
  const offset = layout.offsets[slot] ?? layout.width;
  return slot <= layout.pinned ? offset : offset - view.x;
}

/** Width that fits the widest measured text, clamped to sane bounds. */
export function fitWidth(widths: Iterable<number>, extra = 0): number {
  let widest = 0;
  for (const width of widths) widest = Math.max(widest, width);
  return Math.round(
    Math.min(
      MAX_AUTO_WIDTH,
      Math.max(MIN_COLUMN_WIDTH, widest + TEXT_INSET + extra),
    ),
  );
}

/** A dragged width: never below the minimum. */
export function dragWidth(start: number, delta: number): number {
  return Math.max(MIN_COLUMN_WIDTH, Math.round(start + delta));
}
