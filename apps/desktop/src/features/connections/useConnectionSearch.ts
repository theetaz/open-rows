// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { searchProfiles } from '@openrows/ipc';

/**
 * Ids of the saved connections matching `query` in the core's profile search
 * (name, host, user, database, environment tag or group), refreshed as the
 * user types; `null` while the query is blank. A reply to an older query
 * never replaces a newer one; a failed search keeps the last result.
 */
export function useConnectionSearch(query: string): ReadonlySet<string> | null {
  const [matches, setMatches] = useState<ReadonlySet<string> | null>(null);
  const blank = query.trim() === '';
  useEffect(() => {
    if (blank) return;
    let current = true;
    searchProfiles(query).then(
      (found) => {
        if (current) setMatches(new Set(found.map((profile) => profile.id)));
      },
      () => {
        // The rail keeps showing the last result; the list itself reports load errors.
      },
    );
    return () => {
      current = false;
    };
  }, [query, blank]);
  return blank ? null : matches;
}
