// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState, useSyncExternalStore } from 'react';
import {
  createSavedQuery,
  deleteSavedQuery,
  listSavedQueries,
  renameSavedQuery,
} from '@openrows/ipc';
import type { SavedQueryDto } from '@openrows/ipc';

/** Bumped by every change, so open lists reload. */
let revision = 0;
const listeners = new Set<() => void>();

function changed(): void {
  revision += 1;
  listeners.forEach((listener) => {
    listener();
  });
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Saved queries of one connection; `error` when they could not be read. */
type SavedQueries = {
  queries: readonly SavedQueryDto[];
  loading: boolean;
  error: string | null;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** The connection's saved queries, reloaded after every change. */
export function useSavedQueries(profileId: string | undefined): SavedQueries {
  const current = useSyncExternalStore(subscribe, () => revision);
  const [state, setState] = useState<SavedQueries & { key: string }>({
    key: '',
    queries: [],
    loading: true,
    error: null,
  });
  const key = `${profileId ?? ''}#${String(current)}`;
  useEffect(() => {
    if (!profileId) return;
    let live = true;
    listSavedQueries(profileId).then(
      (queries) => {
        if (live) setState({ key, queries, loading: false, error: null });
      },
      (error: unknown) => {
        if (live)
          setState({ key, queries: [], loading: false, error: message(error) });
      },
    );
    return () => {
      live = false;
    };
  }, [profileId, key]);
  if (!profileId) return { queries: [], loading: false, error: null };
  return state.key.startsWith(`${profileId}#`)
    ? state
    : { queries: [], loading: true, error: null };
}

/** Run ▸ Add to Queries. */
export async function addSavedQuery(
  profileId: string,
  name: string,
  sql: string,
): Promise<void> {
  await createSavedQuery(profileId, name, sql);
  changed();
}

/** Rename a saved query from the sidebar. */
export async function renameQuery(id: string, name: string): Promise<void> {
  await renameSavedQuery(id, name);
  changed();
}

/** Delete a saved query from the sidebar. */
export async function deleteQuery(id: string): Promise<void> {
  await deleteSavedQuery(id);
  changed();
}
