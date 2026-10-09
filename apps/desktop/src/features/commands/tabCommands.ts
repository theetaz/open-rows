// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import { commandForEvent } from './bindings';
import { TAB_COMMANDS } from './registry';
import type { PaneCommand, TabCommand, TableCommand } from './registry';

/**
 * The command handler of the mounted (active) view of one kind: the active
 * query tab or the active table tab. Menu bar, palette and shortcuts outside
 * the view reach it through `run`. With split panes two views are mounted;
 * the last mounted one is the target until another claims it (on focus).
 */
function commandTarget<T extends string>(): {
  useTarget: (handle: (id: T) => void) => () => void;
  run: (id: T) => boolean;
} {
  let current: ((id: T) => void) | null = null;
  const useTarget = (handle: (id: T) => void): (() => void) => {
    const latest = useRef(handle);
    const [target] = useState(() => (id: T): void => {
      latest.current(id);
    });
    useEffect(() => {
      latest.current = handle;
    });
    useEffect(() => {
      current = target;
      return () => {
        if (current === target) current = null;
      };
    }, [target]);
    return useCallback(() => {
      current = target;
    }, [target]);
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
const panes = commandTarget<PaneCommand>();

/**
 * Register the active query tab as the target of tab commands (run, cancel,
 * format, files, export) from the menu bar, palette and shortcuts outside the
 * tab; returns a claim to call when the tab's pane gains focus.
 */
export const useTabCommands = queryTab.useTarget;

/** Run a tab command on the active query tab; `false` when there is none. */
export const runTabCommand = queryTab.run;

/** Register the active table tab as the target of table and edit commands. */
export const useTableCommands = tableTab.useTarget;

/** Run a table command on the active table tab; `false` when there is none. */
export const runTableCommand = tableTab.run;

/** Register the query area as the target of split pane commands. */
export const usePaneCommands = panes.useTarget;

/** Run a split pane command; `false` when no query area is shown. */
export const runPaneCommand = panes.run;

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
