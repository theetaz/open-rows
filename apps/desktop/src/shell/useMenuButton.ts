// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import type { MouseEvent } from 'react';

/**
 * A button that opens a pull-down `Menu` under itself. A click on the button
 * while its menu is open only closes it: the menu dismisses on that
 * pointerdown, and the click that follows must not open it again.
 */
export function useMenuButton(): {
  anchor: HTMLElement | null;
  close: () => void;
  button: {
    'aria-haspopup': 'menu';
    'aria-expanded': boolean;
    onPointerDown: () => void;
    onClick: (event: MouseEvent<HTMLElement>) => void;
  };
} {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const dismissed = useRef(false);
  const close = useCallback(() => {
    setAnchor(null);
  }, []);
  return {
    anchor,
    close,
    button: {
      'aria-haspopup': 'menu',
      'aria-expanded': anchor !== null,
      onPointerDown: () => {
        dismissed.current = anchor !== null;
      },
      onClick: (event) => {
        if (dismissed.current) {
          dismissed.current = false;
          return;
        }
        setAnchor(event.currentTarget);
      },
    },
  };
}
