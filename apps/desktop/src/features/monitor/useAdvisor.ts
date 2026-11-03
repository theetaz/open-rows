// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { monitorAdvice } from '@openrows/ipc';
import type { MonitorTarget, Suggestion } from '@openrows/ipc';
import { t } from './messages/lazyInvestigation';

/** Native evidence refreshes on sample changes and explicit diagnostic completion. */
export function useAdvisor(
  target: MonitorTarget,
  revision: string,
): {
  suggestions: Suggestion[];
  error: string | null;
} {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [error, setError] = useState<string | null>(null);
  const { profileId, database } = target;
  useEffect(() => {
    let alive = true;
    monitorAdvice({ profileId, database }).then(
      (cards) => {
        if (alive) {
          setSuggestions(cards);
          setError(null);
        }
      },
      () => {
        if (alive) {
          setSuggestions([]);
          setError(t('advisorEvidenceUnavailable'));
        }
      },
    );
    return () => {
      alive = false;
    };
  }, [profileId, database, revision]);
  return { suggestions, error };
}
