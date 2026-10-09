// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';
import { keyedStore } from '../../results/keyedStore';
import { parseObjectList, sidebarObjectKey } from './sidebarIdentity';
import type { SidebarObject } from './sidebarIdentity';

/** Device-local, versioned favourites and recent objects, outside profiles. */
const STORAGE_KEY = 'openrows.sidebarObjects.v1';
type SavedObjects = {
  favourites: readonly SidebarObject[];
  recent: readonly SidebarObject[];
};
const empty: SavedObjects = { favourites: [], recent: [] };

function parse(raw: unknown): SavedObjects | null {
  if (!raw || typeof raw !== 'object') return null;
  const value = raw as Record<string, unknown>;
  return {
    favourites: parseObjectList(value.favourites),
    recent: parseObjectList(value.recent).slice(0, 50),
  };
}

const store = keyedStore(STORAGE_KEY, Number.MAX_SAFE_INTEGER, parse);
const scopeKey = (profileId: string, database: string): string =>
  JSON.stringify([profileId, database]);

/** Saved rows of one connection/database, also usable outside React. */
export function readSidebarObjects(
  profileId: string,
  database: string,
): SavedObjects {
  const saved = store.read(scopeKey(profileId, database));
  return saved ?? empty;
}

/** Subscribe to the scoped identity lists without issuing catalog reads. */
export function useSidebarObjects(
  profileId: string,
  database: string,
): SavedObjects {
  return useSyncExternalStore(store.subscribe, () =>
    readSidebarObjects(profileId, database),
  );
}

/** Whether this exact object, including its overload, is starred. */
export function isFavourite(object: SidebarObject): boolean {
  return readSidebarObjects(object.profileId, object.database).favourites.some(
    (item) => sidebarObjectKey(item) === sidebarObjectKey(object),
  );
}

/** Star at the end, or remove the existing favourite. */
export function toggleFavourite(object: SidebarObject): void {
  const saved = readSidebarObjects(object.profileId, object.database);
  const key = sidebarObjectKey(object);
  const favourites = isFavourite(object)
    ? saved.favourites.filter((item) => sidebarObjectKey(item) !== key)
    : [...saved.favourites, object];
  store.write(scopeKey(object.profileId, object.database), {
    ...saved,
    favourites,
  });
}

/** Most recent first, deduplicated and capped at 50 stored entries. */
export function recordRecent(object: SidebarObject): void {
  const saved = readSidebarObjects(object.profileId, object.database);
  const key = sidebarObjectKey(object);
  const recent = [
    object,
    ...saved.recent.filter((item) => sidebarObjectKey(item) !== key),
  ].slice(0, 50);
  store.write(scopeKey(object.profileId, object.database), {
    ...saved,
    recent,
  });
}

/** Clear only this database's recent list. */
export function clearRecent(profileId: string, database: string): void {
  const saved = readSidebarObjects(profileId, database);
  store.write(scopeKey(profileId, database), { ...saved, recent: [] });
}

/** Move a favourite to a zero-based position, preserving every identity. */
export function moveFavourite(object: SidebarObject, position: number): void {
  const saved = readSidebarObjects(object.profileId, object.database);
  if (!isFavourite(object)) return;
  const favourites = saved.favourites.filter(
    (item) => sidebarObjectKey(item) !== sidebarObjectKey(object),
  );
  favourites.splice(
    Math.max(0, Math.min(position, favourites.length)),
    0,
    object,
  );
  store.write(scopeKey(object.profileId, object.database), {
    ...saved,
    favourites,
  });
}
