// SPDX-License-Identifier: Apache-2.0
import { useCallback, useSyncExternalStore } from 'react';
import { emptyLayout, togglePin } from './columnLayout';
import type { ColumnLayout } from './columnLayout';

/** Web-storage key of every grid's column layout (device-local view state). */
const STORAGE_KEY = 'openrows.columnLayouts.v1';
/** Layouts kept; the least recently changed are dropped first. */
const CAPACITY = 200;

type Stored = ColumnLayout & { used: number };

const listeners = new Set<() => void>();
let cache: { text: string | null; layouts: Record<string, Stored> } | null =
  null;

const names = (value: unknown): string[] =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];

/** Validate one stored layout, dropping anything malformed. */
function parseLayout(value: unknown): Stored | null {
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
    used: typeof record.used === 'number' ? record.used : 0,
  };
}

function read(): Record<string, Stored> {
  let text: string | null = null;
  try {
    text = localStorage.getItem(STORAGE_KEY);
  } catch {
    text = null;
  }
  if (cache?.text === text) return cache.layouts;
  const layouts: Record<string, Stored> = {};
  try {
    const parsed: unknown = text ? JSON.parse(text) : {};
    if (typeof parsed === 'object' && parsed !== null)
      for (const [key, value] of Object.entries(parsed)) {
        const layout = parseLayout(value);
        if (layout) layouts[key] = layout;
      }
  } catch {
    /* A corrupt entry resets every layout to the defaults. */
  }
  cache = { text, layouts };
  return layouts;
}

/** The stored layout of a grid, or the empty one. */
export function readLayout(key: string): ColumnLayout {
  return read()[key] ?? emptyLayout;
}

/** Store a grid's layout; an empty layout forgets it. */
export function writeLayout(key: string, layout: ColumnLayout): void {
  const empty =
    !layout.order.length &&
    !layout.pinned.length &&
    !Object.keys(layout.widths).length;
  const layouts = Object.fromEntries(
    Object.entries(read()).filter(([name]) => name !== key),
  );
  if (!empty) layouts[key] = { ...layout, used: Date.now() };
  const kept = Object.entries(layouts)
    .sort(([, a], [, b]) => b.used - a.used)
    .slice(0, CAPACITY);
  const text = JSON.stringify(Object.fromEntries(kept));
  try {
    localStorage.setItem(STORAGE_KEY, text);
    cache = null;
  } catch {
    cache = { text: null, layouts: Object.fromEntries(kept) };
  }
  listeners.forEach((listener) => {
    listener();
  });
}

/** Forget a grid's widths, order and pins (palette: Reset Columns). */
export function resetLayout(key: string): void {
  writeLayout(key, emptyLayout);
}

/** Pin or unpin one column of a grid (palette: Pin Column). */
export function togglePinned(key: string, name: string): void {
  writeLayout(key, togglePin(readLayout(key), name));
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** A grid's live layout and a function to change it. */
export function useColumnLayout(
  key: string,
): [ColumnLayout, (change: (layout: ColumnLayout) => ColumnLayout) => void] {
  const layout = useSyncExternalStore(subscribe, () => readLayout(key));
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
