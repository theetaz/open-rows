// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import type { MouseEvent } from 'react';

/**
 * A pull-down button's open state: the element its `Menu` hangs from. A
 * press on the button while the menu is open closes it as an outside press,
 * so the click that follows must not open it again.
 */
export function usePullDown(): {
  anchor: HTMLElement | null;
  button: {
    'aria-expanded': boolean;
    onPointerDown: () => void;
    onClick: (event: MouseEvent<HTMLElement>) => void;
  };
  close: () => void;
} {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const closing = useRef(false);
  const close = useCallback(() => {
    setAnchor(null);
  }, []);
  return {
    anchor,
    close,
    button: {
      'aria-expanded': anchor !== null,
      onPointerDown: () => {
        closing.current = anchor !== null;
      },
      onClick: (event) => {
        if (closing.current) closing.current = false;
        else setAnchor(event.currentTarget);
      },
    },
  };
}
