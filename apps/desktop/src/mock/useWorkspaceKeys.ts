// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { shortcut } from './shortcuts';
/** Dispatch exact mock shortcuts only when the workspace owns keyboard input. */
export function useWorkspaceKeys(actions: {
  enabled: boolean;
  command: (id: string) => void;
  connection: (index: number) => void;
}): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (!actions.enabled || document.querySelector('dialog[open]')) return;
      const command = shortcut(event);
      if (command === undefined) return;
      event.preventDefault();
      if (typeof command === 'number') actions.connection(command);
      else actions.command(command);
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, [actions]);
}
