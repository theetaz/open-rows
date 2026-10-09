// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { commandForEvent } from './bindings';
import { TAB_COMMANDS } from './registry';
import type { TabCommand, TableCommand } from './registry';

/**
 * The command handler of the mounted (active) view of one kind: the active
 * query tab or the active table tab. Menu bar, palette and shortcuts outside
 * the view reach it through `run`.
 */
function commandTarget<T extends string>(): {
  useTarget: (handle: (id: T) => void) => void;
  run: (id: T) => boolean;
} {
  let current: ((id: T) => void) | null = null;
  const useTarget = (handle: (id: T) => void): void => {
    const latest = useRef(handle);
    useEffect(() => {
      latest.current = handle;
    });
    useEffect(() => {
      const target = (id: T): void => {
        latest.current(id);
      };
      current = target;
      return () => {
        if (current === target) current = null;
      };
    }, []);
  };
  return {
    useTarget,
    run: (id) => {
      if (!current) return false;
      current(id);
      return true;
    },
  };
}

const queryTab = commandTarget<TabCommand>();
const tableTab = commandTarget<TableCommand>();

/**
 * Register the active query tab as the target of tab commands (run, cancel,
 * format, files, export) from the menu bar, palette and shortcuts outside the tab.
 */
export const useTabCommands = queryTab.useTarget;

/** Run a tab command on the active query tab; `false` when there is none. */
export const runTabCommand = queryTab.run;

/** Register the active table tab as the target of table and edit commands. */
export const useTableCommands = tableTab.useTarget;

/** Run a table command on the active table tab; `false` when there is none. */
export const runTableCommand = tableTab.run;

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
