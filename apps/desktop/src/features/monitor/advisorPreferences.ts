// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';

type Preferences = Record<string, number>;
function read(key: string): Preferences {
  try {
    const text = localStorage.getItem(key);
    if (!text || text.length > 65_536) return {};
    const value: unknown = JSON.parse(text);
    if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
    return Object.fromEntries(
      Object.entries(value)
        .slice(-128)
        .filter(
          ([id, expiry]) =>
            id.length <= 512 &&
            typeof expiry === 'number' &&
            Number.isFinite(expiry),
        ),
    );
  } catch {
    return {};
  }
}
/** Persist only rule/identity dismissals in this connection/database, never SQL/evidence. */
export function useAdvisorPreferences(scope: string): {
  hidden: (id: string, now: number) => boolean;
  hide: (id: string, until: number) => void;
  reset: () => void;
  error: string | null;
} {
  const key = `openrows-advisor-v1:${scope}`;
  const [preferences, setPreferences] = useState(() => read(key));
  const [error, setError] = useState<string | null>(null);
  const save = (next: Preferences): void => {
    setPreferences(next);
    try {
      localStorage.setItem(key, JSON.stringify(next));
      setError(null);
    } catch {
      setError(
        'Dismissal applies for this view; local storage is unavailable.',
      );
    }
  };
  return {
    hidden: (id, now) => preferences[id] === -1 || (preferences[id] ?? 0) > now,
    hide: (id, until) => {
      const entries: [string, number][] = [
        ...Object.entries(preferences).filter(([key]) => key !== id),
        [id, until],
      ];
      const next: Preferences = {};
      for (const [key, value] of entries.slice(-128)) next[key] = value;
      save(next);
    },
    reset: () => {
      save({});
    },
    error,
  };
}
