// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { clearHistory, deleteHistory, searchHistory } from '@openrows/ipc';
import type { HistoryItemDto } from '@openrows/ipc';
import { announceHistoryChange, useHistoryRevision } from './historyEvents';

/** Typing waits this long before the search runs. */
const SEARCH_DELAY_MS = 150;
/** Entries listed at most; narrowing the text reaches older ones. */
const LIMIT = 300;

type HistoryFilter = {
  /** The saved connection to list; `null` lists every connection. */
  profileId: string | null;
  /** Free text; blank lists everything. */
  text: string;
  /** `false` lists nothing (no connection selected yet). */
  enabled: boolean;
};

export type HistoryState = {
  items: readonly HistoryItemDto[];
  /** The last failure, shown above the list. */
  problem: string | null;
  remove: (id: number) => Promise<void>;
  clear: (profileId: string | null) => Promise<void>;
};

const NOTHING: readonly HistoryItemDto[] = [];

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * The history matching `filter`. The list refetches when the filter or the
 * local history changes; a stale answer never replaces a newer one.
 */
export function useHistory(filter: HistoryFilter): HistoryState {
  const revision = useHistoryRevision();
  const [items, setItems] = useState<readonly HistoryItemDto[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const { profileId, text, enabled } = filter;
  useEffect(() => {
    if (!enabled) return undefined;
    let live = true;
    const timer = setTimeout(() => {
      searchHistory({ profileId, text, limit: LIMIT }).then(
        (found) => {
          if (!live) return;
          setItems(found);
          setProblem(null);
        },
        (error: unknown) => {
          if (!live) return;
          setItems([]);
          setProblem(message(error));
        },
      );
    }, SEARCH_DELAY_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [enabled, profileId, text, revision]);
  const change = async (action: () => Promise<unknown>): Promise<void> => {
    try {
      await action();
      setProblem(null);
      announceHistoryChange();
    } catch (error) {
      setProblem(message(error));
    }
  };
  return {
    items: enabled ? items : NOTHING,
    problem,
    remove: (id) => change(() => deleteHistory(id)),
    clear: (profile) => change(() => clearHistory(profile)),
  };
}
