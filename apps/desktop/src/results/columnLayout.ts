// SPDX-License-Identifier: Apache-2.0
import { MIN_COLUMN_WIDTH } from '@openrows/grid';

/** A grid's own arrangement of its columns, keyed by column name. */
export type ColumnLayout = {
  /** Widths the user dragged or auto-sized. */
  widths: Readonly<Record<string, number>>;
  /** Display order; names not listed follow in result order. */
  order: readonly string[];
  /** Frozen columns; they lead the display order. */
  pinned: readonly string[];
};

export const emptyLayout: ColumnLayout = { widths: {}, order: [], pinned: [] };

/** Result column names in display order: pinned first, then the arranged rest. */
export function arrange(
  layout: ColumnLayout,
  names: readonly string[],
): string[] {
  const known = new Set(names);
  const pinned = layout.pinned.filter((name) => known.has(name));
  const placed = new Set(pinned);
  const ordered = layout.order.filter(
    (name) => known.has(name) && !placed.has(name),
  );
  for (const name of ordered) placed.add(name);
  return [...pinned, ...ordered, ...names.filter((name) => !placed.has(name))];
}

/** How many leading `shown` columns are pinned (a prefix, as the grid requires). */
export function pinnedCount(
  layout: ColumnLayout,
  shown: readonly string[],
): number {
  let count = 0;
  while (count < shown.length && layout.pinned.includes(shown[count] ?? ''))
    count++;
  return count;
}

/** Store one column's width. */
export function withWidth(
  layout: ColumnLayout,
  name: string,
  width: number,
): ColumnLayout {
  return {
    ...layout,
    widths: {
      ...layout.widths,
      [name]: Math.max(MIN_COLUMN_WIDTH, Math.round(width)),
    },
  };
}

/**
 * Move the column shown at `from` to `to`. Pinned columns stay among the
 * pinned ones and the rest stay after them.
 */
export function moved(
  layout: ColumnLayout,
  shown: readonly string[],
  from: number,
  to: number,
): ColumnLayout {
  const name = shown[from];
  if (name === undefined) return layout;
  const pins = pinnedCount(layout, shown);
  const [low, high] = from < pins ? [0, pins - 1] : [pins, shown.length - 1];
  const order = shown.filter((_, index) => index !== from);
  order.splice(Math.min(high, Math.max(low, to)), 0, name);
  const rest = layout.order.filter((item) => !order.includes(item));
  return {
    ...layout,
    order: [...order, ...rest],
    pinned: order.slice(0, pins),
  };
}

/** Pin a column after the pinned ones, or unpin it to the front of the rest. */
export function togglePin(layout: ColumnLayout, name: string): ColumnLayout {
  if (layout.pinned.includes(name))
    return {
      ...layout,
      pinned: layout.pinned.filter((item) => item !== name),
      order: [name, ...layout.order.filter((item) => item !== name)],
    };
  return { ...layout, pinned: [...layout.pinned, name] };
}

/** A width a user set, else `fallback`. */
export function widthOf(
  layout: ColumnLayout,
  name: string,
  fallback: number,
): number {
  return layout.widths[name] ?? fallback;
}
