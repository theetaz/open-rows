// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { checkCatalog } from '@openrows/ipc';

/** Focus changes closer together than this check the server only once. */
const FINGERPRINT_INTERVAL_MS = 5000;

/**
 * When the window regains focus, ask whether the schema changed outside
 * the app (M1-16): the Rust side compares a cheap catalog fingerprint and,
 * when it moved, marks the cache stale and emits `catalog-changed`, which
 * re-reads what the tree shows.
 */
export function useSchemaFingerprint(
  profileId: string | null,
  database: string,
): void {
  useEffect(() => {
    if (!profileId || !database) return;
    let last = Number.NEGATIVE_INFINITY;
    const check = (): void => {
      const now = performance.now();
      if (now - last < FINGERPRINT_INTERVAL_MS) return;
      last = now;
      checkCatalog(profileId, database).catch(() => undefined);
    };
    check();
    window.addEventListener('focus', check);
    return () => {
      window.removeEventListener('focus', check);
    };
  }, [profileId, database]);
}
