// SPDX-License-Identifier: Apache-2.0
import { useCallback, useId, useRef, useState } from 'react';
import type { RefObject } from 'react';
type MenuState = {
  opener: RefObject<HTMLButtonElement | null>;
  id: string;
  open: boolean;
  close: () => void;
  toggle: () => void;
};
/** Shared menu disclosure state with a stable opener for focus restoration. */
export function useSidebarMenu(): MenuState {
  const opener = useRef<HTMLButtonElement>(null);
  const id = useId();
  const [open, setOpen] = useState(false);
  const close = useCallback(() => {
    setOpen(false);
    opener.current?.focus();
  }, []);
  const toggle = (): void => {
    setOpen(!open);
  };
  return { opener, id, open, close, toggle };
}
