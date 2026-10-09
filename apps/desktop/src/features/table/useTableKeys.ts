// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import type { TableActions } from './useTableTab';

const FIRST_VALUE = 'input[aria-label^="Value of filter"]';

/** Focus the first filter value, opening the filter bar first when closed. */
export function focusFilter(
  root: HTMLElement | null,
  actions: TableActions,
): void {
  const input = root?.querySelector<HTMLInputElement>(FIRST_VALUE);
  if (input) {
    input.focus();
    return;
  }
  actions.toggleFilters();
  requestAnimationFrame(() => {
    root?.querySelector<HTMLInputElement>(FIRST_VALUE)?.focus();
  });
}

/** ⌘F focuses the filter bar; ⌘[ and ⌘] page back and forward. */
export function useTableKeys(
  root: RefObject<HTMLElement | null>,
  actions: TableActions,
): void {
  const latest = useRef(actions);
  useEffect(() => {
    latest.current = actions;
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (!event.metaKey || event.shiftKey || event.altKey || event.ctrlKey)
        return;
      if (event.isComposing || document.querySelector('dialog[open]')) return;
      const commands: Readonly<Record<string, (() => void) | undefined>> = {
        f: () => {
          focusFilter(root.current, latest.current);
        },
        '[': () => {
          latest.current.page(-1);
        },
        ']': () => {
          latest.current.page(1);
        },
      };
      const command = commands[event.key.toLowerCase()];
      if (!command) return;
      event.preventDefault();
      command();
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, [root]);
}
