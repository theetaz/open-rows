// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { onMenuCommand, syncMenu } from '@openrows/ipc';
import type { MenuItemState } from '@openrows/ipc';
import { useBindings } from './bindings';
import type { Bindings } from './bindings';
import { formatCombo } from './keys';
import { COMMANDS } from './registry';

/**
 * A menu event this soon after a key press the webview already handled is
 * that same press reaching the menu bar, and is dropped.
 */
const ECHO_MS = 250;

/** Menu item states for every command: enabled flags and shortcuts. */
export function menuStates(
  bindings: Bindings,
  enabled: Readonly<Record<string, boolean>>,
): MenuItemState[] {
  return COMMANDS.map(({ id }) => {
    const combo = bindings[id];
    return {
      id,
      enabled: enabled[id] ?? true,
      accelerator: combo ? formatCombo(combo) : null,
    };
  });
}

/** When the last ⌘/⌃ key press was handled inside the webview. */
let handledAt = 0;

/** Record a shortcut handled by a window-level key listener. */
export function markKeyHandled(): void {
  handledAt = performance.now();
}

/** Record ⌘/⌃ presses that handlers inside the page consumed. */
function useHandledKeys(): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.defaultPrevented)
        markKeyHandled();
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, []);
}

/**
 * Route native menu items to `command` and keep the menu bar's enabled state
 * and shortcuts in step with the app.
 */
export function useMenuBridge(
  command: (id: string) => void,
  enabled: Readonly<Record<string, boolean>>,
): void {
  const latest = useRef(command);
  useEffect(() => {
    latest.current = command;
  });
  useHandledKeys();
  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    onMenuCommand((id) => {
      if (performance.now() - handledAt < ECHO_MS) return;
      latest.current(id);
    }).then(
      (unlisten) => {
        if (live) stop = unlisten;
        else unlisten();
      },
      () => undefined,
    );
    return () => {
      live = false;
      stop?.();
    };
  }, []);
  const bindings = useBindings();
  const states = JSON.stringify(menuStates(bindings, enabled));
  useEffect(() => {
    syncMenu(JSON.parse(states) as MenuItemState[]).catch(() => undefined);
  }, [states]);
}
