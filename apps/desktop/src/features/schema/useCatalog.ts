// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import {
  listDatabases,
  listObjects,
  listSchemas,
  searchCatalog,
} from '@openrows/ipc';
import type { DatabaseDto, ObjectDto, SchemaDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { defaultSchema } from './catalog';
import type { ObjectGroup } from './catalog';
import { useNavigation } from './useNavigation';
import type { NavigationState } from './useNavigation';
import { useResources } from './useResources';
import type { Resource, Resources } from './useResources';

/** The active connection's real catalog, read lazily on metadata sessions. */
export type Catalog = Omit<NavigationState, 'chosenSchema'> & {
  /** Why there is nothing to browse, or `null` when connected. */
  offline: string | null;
  databases: Resource<DatabaseDto[]>;
  schemas: Resource<SchemaDto[]>;
  /** Selected schema; empty until schemas are known. */
  schema: string;
  objects: (group: ObjectGroup) => Resource<ObjectDto[]>;
  loadObjects: (group: ObjectGroup) => void;
  search: Resource<ObjectDto[]>;
  loadSearch: () => void;
  /** Forget everything read from the active database and read it again. */
  refresh: () => void;
};

const offlineReason = (connections: Connections): string | null => {
  const profile = connections.active;
  if (!profile) return 'Select a connection to browse its objects';
  const status = connections.statusOf(profile.id);
  if (status.state === 'connected') return null;
  if (status.state === 'connecting') return 'Connecting…';
  return 'Connect to browse objects';
};

/** Real databases, schemas and objects of the selected connection. */
export function useCatalog(connections: Connections): Catalog {
  const resources = useResources();
  const status = connections.active
    ? connections.statusOf(connections.active.id)
    : undefined;
  const connected = status?.state === 'connected' ? status : undefined;
  const profileId = connected ? (connections.active?.id ?? null) : null;
  const navigation = useNavigation(profileId, connected?.server.database ?? '');
  const { database, chosenSchema, ...rest } = navigation;
  const prefix = `${profileId ?? ''}\n${connected?.sessionId ?? ''}\n${database}\n`;
  const reads = useReads(resources, prefix, profileId, database);
  const schema = pickSchema(reads.schemas, chosenSchema, rest.systemSchemas);
  return {
    ...rest,
    database,
    offline: offlineReason(connections),
    ...reads,
    schema,
    objects: (group) =>
      resources.get(`${prefix}objects\n${schema}\n${group.id}`),
    loadObjects: (group) => {
      if (!profileId || !schema) return;
      resources.load(`${prefix}objects\n${schema}\n${group.id}`, () =>
        listObjects(profileId, database, schema, group.kinds),
      );
    },
    refresh: () => {
      resources.invalidate(prefix);
    },
  };
}

function pickSchema(
  schemas: Resource<SchemaDto[]>,
  chosen: string | undefined,
  system: boolean,
): string {
  if (schemas.status !== 'ready') return '';
  const visible = schemas.data.filter((schema) => system || !schema.system);
  return visible.some((schema) => schema.name === chosen) && chosen
    ? chosen
    : defaultSchema(visible);
}

/** Databases and schemas load as soon as a database is active; search on demand. */
function useReads(
  resources: Resources,
  prefix: string,
  profileId: string | null,
  database: string,
): Pick<Catalog, 'databases' | 'schemas' | 'search' | 'loadSearch'> {
  const { load } = resources;
  const databases = resources.get<DatabaseDto[]>(`${prefix}databases`);
  const schemas = resources.get<SchemaDto[]>(`${prefix}schemas`);
  useEffect(() => {
    if (!profileId || !database) return;
    load(`${prefix}databases`, () => listDatabases(profileId, database));
    load(`${prefix}schemas`, () => listSchemas(profileId, database));
  }, [load, prefix, profileId, database, databases.status, schemas.status]);
  return {
    databases,
    schemas,
    search: resources.get<ObjectDto[]>(`${prefix}search`),
    loadSearch: () => {
      if (!profileId || !database) return;
      load(`${prefix}search`, () => searchCatalog(profileId, database));
    },
  };
}
