// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { markKeyHandled } from '../commands';
import { shortcut } from './shortcuts';
/**
 * Dispatch bound shortcuts when the workspace owns keyboard input and no
 * focused control (editor, table, sidebar) handled the press first.
 */
export function useWorkspaceKeys(actions: {
  enabled: boolean;
  command: (id: string) => void;
  connection: (index: number) => void;
}): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (!actions.enabled || event.defaultPrevented) return;
      if (document.querySelector('dialog[open]')) return;
      const command = shortcut(event);
      if (command === undefined) return;
      event.preventDefault();
      markKeyHandled();
      if (typeof command === 'number') actions.connection(command);
      else actions.command(command);
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, [actions]);
}
