// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';

/**
 * Tells the History list that the local history changed: a run finished, or
 * entries were deleted or cleared. Lists refetch when the revision moves.
 */
let revision = 0;
const listeners = new Set<() => void>();

/** Note that the local history changed and wake every History list. */
export function announceHistoryChange(): void {
  revision += 1;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** The current history revision; changes after every announcement. */
export function useHistoryRevision(): number {
  return useSyncExternalStore(subscribe, () => revision);
}
