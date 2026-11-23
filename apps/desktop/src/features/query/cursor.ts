// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { t } from './lazyMessages';

/** Cursor of the editor: 1-based line and column. */
export type CursorPosition = { line: number; column: number };

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
  let position: CursorPosition = { line: 1, column: 1 };
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

/** `Ln N, Col M` for the status line. */
export function cursorText(position: CursorPosition): string {
  return t('lineColumn', {
    line: String(position.line),
    column: String(position.column),
  });
}

/** The current cursor position, re-rendering on every move. */
export function useCursor(store: CursorStore): CursorPosition {
  return useSyncExternalStore(store.subscribe, store.get);
}
