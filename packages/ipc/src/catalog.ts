// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  DatabaseDto,
  ObjectDto,
  ObjectKindDto,
  SchemaDto,
} from './generated/bindings';
import { unwrap } from './connections';

/** Databases on the server of a connected profile. */
export function listDatabases(
  profileId: string,
  database: string,
): Promise<DatabaseDto[]> {
  return unwrap(commands.catalogDatabases(profileId, database));
}

/** Schemas of one database, user schemas first. */
export function listSchemas(
  profileId: string,
  database: string,
): Promise<SchemaDto[]> {
  return unwrap(commands.catalogSchemas(profileId, database));
}

/** Objects of the given kinds in one schema, ordered by name. */
export function listObjects(
  profileId: string,
  database: string,
  schema: string,
  kinds: ObjectKindDto[],
): Promise<ObjectDto[]> {
  return unwrap(commands.catalogObjects(profileId, database, schema, kinds));
}

/** Bounded name index of one database for quick open. */
export function searchCatalog(
  profileId: string,
  database: string,
): Promise<ObjectDto[]> {
  return unwrap(commands.catalogSearch(profileId, database));
}
