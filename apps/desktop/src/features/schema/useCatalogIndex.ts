// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { onCatalogChanged, searchCatalog } from '@openrows/ipc';
import type { ObjectDto } from '@openrows/ipc';
import type { Resource, Resources } from './useResources';

/** What a name search narrows to. */
export type IndexQuery = {
  text: string;
  /** Only this schema's objects; `null` searches every user schema. */
  schema: string | null;
  limit: number;
};

/** The database's prebuilt name index (Rust side, M1-16). */
export type CatalogIndex = {
  /** Number of indexed objects once the index is read. */
  index: Resource<number>;
  /** Read the index (from its snapshot at once, refreshed in background). */
  loadIndex: () => void;
  /** Ranked matches: exact, prefix, qualified prefix, substring, subsequence. */
  searchIndex: (query: IndexQuery) => Promise<ObjectDto[]>;
};

/** The index of `database`, cached under `scoped` with the other reads. */
export function useCatalogIndex(
  resources: Resources,
  scoped: string,
  target: { profileId: string | null; database: string },
): CatalogIndex {
  const { profileId, database } = target;
  const key = `${scoped}index`;
  const { load } = resources;
  const loadIndex = useCallback(() => {
    if (!profileId || !database) return;
    load(key, async (fresh) => {
      const request = { query: '', schema: null, limit: 0, fresh };
      const reply = await searchCatalog(profileId, database, request);
      return { data: reply.total, stale: reply.stale };
    });
  }, [load, key, profileId, database]);
  const searchIndex = useCallback(
    async ({ text, schema, limit }: IndexQuery): Promise<ObjectDto[]> => {
      if (!profileId || !database) return [];
      const request = { query: text, schema, limit, fresh: false };
      return (await searchCatalog(profileId, database, request)).objects;
    },
    [profileId, database],
  );
  return { index: resources.get<number>(key), loadIndex, searchIndex };
}

/** The latest search and the query it answered. */
type Found = { key: string; resource: Resource<ObjectDto[]> };

const failure = (error: unknown): Resource<never> => ({
  status: 'error',
  message: error instanceof Error ? error.message : String(error),
});

/**
 * Matches of `query` in the database's index, searched in Rust (off the UI
 * thread) and again whenever the index refreshes; `null` while `query` is.
 * With `keep`, the previous matches stay shown while the next search runs
 * (a tree that should not flicker); otherwise they are `loading`, so a
 * palette never chooses a match of an older query.
 */
export function useIndexSearch(
  catalog: CatalogIndex,
  query: IndexQuery | null,
  keep = false,
): Resource<ObjectDto[]> | null {
  const [found, setFound] = useState<Found | null>(null);
  const { index, loadIndex, searchIndex } = catalog;
  const key = query ? JSON.stringify(query) : '';
  const on = query !== null;
  useEffect(() => {
    if (on) loadIndex();
  }, [on, loadIndex]);
  useEffect(() => {
    if (!key || index.status !== 'ready') return;
    let live = true;
    const settle = (resource: Resource<ObjectDto[]>): void => {
      if (live) setFound({ key, resource });
    };
    searchIndex(JSON.parse(key) as IndexQuery).then(
      (objects) => {
        settle({ status: 'ready', data: objects, refreshing: false });
      },
      (error: unknown) => {
        settle(failure(error));
      },
    );
    return () => {
      live = false;
    };
  }, [key, index, searchIndex]);
  if (!query) return null;
  if (index.status === 'error') return index;
  const current = found && (keep || found.key === key);
  return current ? found.resource : { status: 'loading' };
}

/** Run `refresh` whenever DDL ran on `profileId` (any of its databases). */
export function useCatalogChanges(
  profileId: string | null,
  refresh: () => void,
): void {
  useEffect(() => {
    if (!profileId) return;
    let live = true;
    let stop: (() => void) | undefined;
    onCatalogChanged((event) => {
      if (event.profileId === profileId) refresh();
    }).then(
      (unlisten) => {
        if (live) stop = unlisten;
        else unlisten();
      },
      () => undefined,
    );
    return () => {
      live = false;
      stop?.();
    };
  }, [profileId, refresh]);
}
