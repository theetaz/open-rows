// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
const query = '(prefers-reduced-motion: reduce)';
function subscribe(notify: () => void): () => void {
  const media = window.matchMedia(query);
  media.addEventListener('change', notify);
  const observer = new MutationObserver(notify);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-motion'],
  });
  return () => {
    media.removeEventListener('change', notify);
    observer.disconnect();
  };
}
/**
 * Follow the OS motion preference and the app's Reduce motion setting
 * (`data-motion="reduced"` on the root), including changes while a popover
 * is open.
 */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribe,
    () =>
      window.matchMedia(query).matches ||
      document.documentElement.dataset.motion === 'reduced',
    () => false,
  );
}
