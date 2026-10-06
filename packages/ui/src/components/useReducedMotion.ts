// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
const query = '(prefers-reduced-motion: reduce)';
function subscribe(notify: () => void): () => void {
  const media = window.matchMedia(query);
  media.addEventListener('change', notify);
  return () => {
    media.removeEventListener('change', notify);
  };
}
/** Follow the OS motion preference, including changes while a popover is open. */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia(query).matches,
    () => false,
  );
}
