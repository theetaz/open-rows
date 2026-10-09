// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { commandForEvent } from './bindings';
import { TAB_COMMANDS } from './registry';
import type { TabCommand } from './registry';

/** The mounted (active) query tab's command handler. */
let current: ((id: TabCommand) => void) | null = null;

/**
 * Register the active query tab as the target of tab commands (run, cancel,
 * format, files) from the menu bar, palette and shortcuts outside the tab.
 */
export function useTabCommands(handle: (id: TabCommand) => void): void {
  const latest = useRef(handle);
  useEffect(() => {
    latest.current = handle;
  });
  useEffect(() => {
    const target = (id: TabCommand): void => {
      latest.current(id);
    };
    current = target;
    return () => {
      if (current === target) current = null;
    };
  }, []);
}

/** Run a tab command on the active query tab; `false` when there is none. */
export function runTabCommand(id: TabCommand): boolean {
  if (!current) return false;
  current(id);
  return true;
}

/** The tab command bound to a key press, unless something handled it already. */
export function tabCommandOf(event: {
  key: string;
  code: string;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  ctrlKey: boolean;
  defaultPrevented: boolean;
}): TabCommand | undefined {
  if (event.defaultPrevented) return undefined;
  return commandForEvent(event, TAB_COMMANDS) as TabCommand | undefined;
}
