// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';

/** Persist visibility only; folding a connection never closes its sessions. */
export function useWorkspaceCollapse(id: string): [boolean, () => void] {
  const key = `openrows.workspace.collapsed.${id}`;
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(key) === 'true';
    } catch {
      return false;
    }
  });
  const toggle = (): void => {
    const next = !collapsed;
    setCollapsed(next);
    try {
      localStorage.setItem(key, String(next));
    } catch {
      /* Appearance persistence is optional when storage is unavailable. */
    }
  };
  return [collapsed, toggle];
}
