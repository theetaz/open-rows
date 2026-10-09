// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands, events } from './generated/bindings';
import type {
  CatalogChanged,
  CatalogReply,
  CatalogSearchReply,
  CatalogSearchRequest,
  DatabaseDto,
  ObjectsReply,
  ObjectsRequest,
  SchemaDto,
} from './generated/bindings';
import { unwrap } from './connections';

// Catalog reads are served by the schema cache (M1-16): a `stale` reply came
// from a persisted snapshot or was invalidated, so show it and read it again
// with `fresh`.

/** Databases on the server of a connected profile. */
export function listDatabases(
  profileId: string,
  database: string,
  fresh: boolean,
): Promise<CatalogReply<DatabaseDto[]>> {
  return unwrap(commands.catalogDatabases(profileId, database, fresh));
}

/** Schemas of one database, user schemas first. */
export function listSchemas(
  profileId: string,
  database: string,
  fresh: boolean,
): Promise<CatalogReply<SchemaDto[]>> {
  return unwrap(commands.catalogSchemas(profileId, database, fresh));
}

/**
 * Objects of the given kinds in one schema, ordered by name: the first
 * `limit` (all without one) and how many there are in all.
 */
export function listObjects(
  profileId: string,
  database: string,
  section: Omit<ObjectsRequest, 'fresh'>,
  fresh: boolean,
): Promise<ObjectsReply> {
  return unwrap(
    commands.catalogObjects(profileId, database, { ...section, fresh }),
  );
}

/** Ranked search over one database's prebuilt name index (Rust side). */
export function searchCatalog(
  profileId: string,
  database: string,
  request: CatalogSearchRequest,
): Promise<CatalogSearchReply> {
  return unwrap(commands.catalogSearch(profileId, database, request));
}

/**
 * Compare the server's schema fingerprint with the last one seen; `true`
 * (and a `catalog-changed` event) when DDL ran outside the app since.
 */
export function checkCatalog(
  profileId: string,
  database: string,
): Promise<boolean> {
  return unwrap(commands.catalogCheck(profileId, database));
}

/** Mark everything cached for a connection stale before a manual refresh. */
export function invalidateCatalog(profileId: string): Promise<void> {
  return commands.catalogInvalidate(profileId);
}

/** Subscribe to DDL run on any connection; resolves to the unsubscribe function. */
export function onCatalogChanged(
  listener: (event: CatalogChanged) => void,
): Promise<() => void> {
  if (!isTauri()) return Promise.resolve(() => {});
  return events.catalogChanged.listen((event) => {
    listener(event.payload);
  });
}
