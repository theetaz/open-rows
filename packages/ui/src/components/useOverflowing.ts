// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';

/** Whether the element's content is wider than its box; re-checked on resize and when `count` changes. */
export function useOverflowing(
  element: React.RefObject<HTMLElement | null>,
  count: number,
): boolean {
  const [overflowing, setOverflowing] = useState(false);
  useEffect(() => {
    const node = element.current;
    if (!node) return;
    const check = (): void => {
      setOverflowing(node.scrollWidth > node.clientWidth + 1);
    };
    check();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(check);
    observer.observe(node);
    return () => {
      observer.disconnect();
    };
  }, [element, count, setOverflowing]);
  return overflowing;
}
