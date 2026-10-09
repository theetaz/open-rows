// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';

/** Text fields keep their own copy behaviour, e.g. the inline cell editor. */
function editable(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

/**
 * ⌘C (or Edit ▸ Copy) over a grid copies its selection: `copy` reads the
 * cells and writes the clipboard itself, since reading is asynchronous.
 */
export function useCopyEvent(
  host: React.RefObject<HTMLElement | null>,
  copy: () => void,
): void {
  const latest = useRef(copy);
  useEffect(() => {
    latest.current = copy;
  });
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const listener = (event: ClipboardEvent): void => {
      if (editable(event.target)) return;
      event.preventDefault();
      latest.current();
    };
    element.addEventListener('copy', listener);
    return () => {
      element.removeEventListener('copy', listener);
    };
  }, [host]);
}
