// SPDX-License-Identifier: Apache-2.0
import { useCallback, useSyncExternalStore } from 'react';
import { keyedStore } from '../../results/keyedStore';
import {
  emptyPresentation,
  parsePresentation,
  savedWith,
} from './presentation';
import type { SavedFilter, TablePresentation } from './presentation';

/** Web-storage key of every table's remembered presentation (device-local). */
const STORAGE_KEY = 'openrows.tablePresentation.v1';
/** Tables kept; the least recently changed are dropped first. */
const CAPACITY = 200;

const store = keyedStore(STORAGE_KEY, CAPACITY, parsePresentation);

/** The remembered presentation of a table (key: `tableLayoutKey`). */
export function readPresentation(key: string): TablePresentation {
  return store.read(key) ?? emptyPresentation;
}

/** Remember a table's presentation; the defaults forget it. */
export function writePresentation(
  key: string,
  presentation: TablePresentation,
): void {
  const empty =
    presentation.striped == null &&
    !presentation.search &&
    !presentation.hidden.length &&
    !presentation.sort &&
    !presentation.rows.length &&
    !presentation.applied.length &&
    !presentation.saved.length;
  store.write(key, empty ? undefined : { ...presentation, version: 2 });
}

/** A table's saved filters, and functions to add (or replace) and remove one. */
export function useSavedFilters(key: string): {
  saved: readonly SavedFilter[];
  save: (filter: SavedFilter) => void;
  remove: (name: string) => void;
} {
  const saved = useSyncExternalStore(
    store.subscribe,
    () => readPresentation(key).saved,
  );
  const change = useCallback(
    (update: (saved: readonly SavedFilter[]) => readonly SavedFilter[]) => {
      const current = readPresentation(key);
      writePresentation(key, { ...current, saved: update(current.saved) });
    },
    [key],
  );
  return {
    saved,
    save: (filter) => {
      change((current) => savedWith(current, filter));
    },
    remove: (name) => {
      change((current) => current.filter((item) => item.name !== name));
    },
  };
}
