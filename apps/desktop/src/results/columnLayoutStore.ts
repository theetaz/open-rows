// SPDX-License-Identifier: Apache-2.0
import { useCallback, useSyncExternalStore } from 'react';
import { emptyLayout, togglePin } from './columnLayout';
import type { ColumnLayout } from './columnLayout';
import { keyedStore } from './keyedStore';

/** Web-storage key of every grid's column layout (device-local view state). */
const STORAGE_KEY = 'openrows.columnLayouts.v1';
/** Layouts kept; the least recently changed are dropped first. */
const CAPACITY = 200;

const names = (value: unknown): string[] =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];

/** Validate one stored layout, dropping anything malformed. */
function parseLayout(value: unknown): ColumnLayout | null {
  if (typeof value !== 'object' || value === null) return null;
  const record = value as Record<string, unknown>;
  const widths: Record<string, number> = {};
  if (typeof record.widths === 'object' && record.widths !== null)
    for (const [name, width] of Object.entries(record.widths))
      if (typeof width === 'number' && Number.isFinite(width) && width > 0)
        widths[name] = width;
  return {
    widths,
    order: names(record.order),
    pinned: names(record.pinned),
  };
}

const layouts = keyedStore(STORAGE_KEY, CAPACITY, parseLayout);

/** The stored layout of a grid, or the empty one. */
export function readLayout(key: string): ColumnLayout {
  return layouts.read(key) ?? emptyLayout;
}

/** Store a grid's layout; an empty layout forgets it. */
export function writeLayout(key: string, layout: ColumnLayout): void {
  const empty =
    !layout.order.length &&
    !layout.pinned.length &&
    !Object.keys(layout.widths).length;
  layouts.write(key, empty ? undefined : layout);
}

/** Forget a grid's widths, order and pins (palette: Reset Columns). */
export function resetLayout(key: string): void {
  writeLayout(key, emptyLayout);
}

/** Pin or unpin one column of a grid (palette: Pin Column). */
export function togglePinned(key: string, name: string): void {
  writeLayout(key, togglePin(readLayout(key), name));
}

/** A grid's live layout and a function to change it. */
export function useColumnLayout(
  key: string,
): [ColumnLayout, (change: (layout: ColumnLayout) => ColumnLayout) => void] {
  const layout = useSyncExternalStore(layouts.subscribe, () => readLayout(key));
  const change = useCallback(
    (update: (layout: ColumnLayout) => ColumnLayout) => {
      writeLayout(key, update(readLayout(key)));
    },
    [key],
  );
  return [layout, change];
}

/** Layout key of a browsed table. */
export function tableLayoutKey(target: {
  profileId: string;
  database: string;
  schema: string;
  table: string;
}): string {
  return JSON.stringify([
    'table',
    target.profileId,
    target.database,
    target.schema,
    target.table,
  ]);
}

/** Layout key of query results with these columns (same shape, same layout). */
export function resultLayoutKey(columns: readonly { name: string }[]): string {
  return JSON.stringify(['result', ...columns.map((column) => column.name)]);
}
