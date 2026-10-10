// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import type { FilterDto } from '@openrows/ipc';
import type { TableView } from '../model';

/**
 * Filters waiting for a table tab to show them: a foreign-key jump opens
 * (or focuses) the other table's tab, which takes its seed once mounted.
 * Keyed like the table's layout (`tableLayoutKey`).
 */
const pending = new Map<string, readonly FilterDto[]>();
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function changed(): void {
  listeners.forEach((listener) => {
    listener();
  });
}

/** Ask the table tab of `key` to show only the rows `filters` match. */
export function seedFilters(key: string, filters: readonly FilterDto[]): void {
  pending.set(key, filters);
  changed();
}

/** Remove and return the seed of `key`, if any. */
export function takeSeed(key: string): readonly FilterDto[] | undefined {
  const seed = pending.get(key);
  if (seed) {
    pending.delete(key);
    changed();
  }
  return seed;
}

/** The pending seed of `key`, re-rendering when one arrives. */
export function usePendingSeed(key: string): readonly FilterDto[] | undefined {
  return useSyncExternalStore(subscribe, () => pending.get(key));
}

/** The view filtered to a jump's rows: the filters are shown, applied and ANDed. */
export function seededView(
  view: TableView,
  filters: readonly FilterDto[],
): TableView {
  const rows = filters.map((filter, index) => ({
    ...filter,
    id: view.nextRow + index,
    enabled: true,
  }));
  return {
    ...view,
    rows,
    applied: [...filters],
    combinator: 'and',
    groups: undefined,
    appliedGroups: undefined,
    nextRow: view.nextRow + rows.length,
    filtersOpen: true,
    offset: 0,
  };
}
