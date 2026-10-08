// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';

/** Cursor of the editor: 1-based line and column, 0-based document offset. */
export type CursorPosition = { line: number; column: number; location: number };

/**
 * The editor writes the cursor here on every move; only the bottom bar's
 * position text re-renders, not the whole tab.
 */
export type CursorStore = {
  set: (position: CursorPosition) => void;
  subscribe: (listener: () => void) => () => void;
  get: () => CursorPosition;
};

/** A store starting at the document start. */
export function createCursorStore(): CursorStore {
  let position: CursorPosition = { line: 1, column: 1, location: 0 };
  const listeners = new Set<() => void>();
  return {
    set: (next) => {
      position = next;
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    get: () => position,
  };
}

/** `line N, column M, location K` like TablePlus' editor footer. */
export function cursorText(position: CursorPosition): string {
  return `line ${String(position.line)}, column ${String(position.column)}, location ${String(position.location)}`;
}

/** The current cursor position, re-rendering on every move. */
export function useCursor(store: CursorStore): CursorPosition {
  return useSyncExternalStore(store.subscribe, store.get);
}
