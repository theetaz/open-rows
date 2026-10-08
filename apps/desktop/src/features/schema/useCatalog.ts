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
import { useColumns } from './useColumns';
import type { ColumnTree } from './useColumns';
import { useNavigation } from './useNavigation';
import type { NavigationState } from './useNavigation';
import { useResources } from './useResources';
import type { Resource, Resources } from './useResources';

/** The active connection's real catalog, read lazily on metadata sessions. */
export type Catalog = Omit<NavigationState, 'chosenSchema'> &
  ColumnTree & {
    /** Why there is nothing to browse, or `null` when connected. */
    offline: string | null;
    /** Active database; empty when the connection has none open. */
    database: string;
    /** Names of the connection's opened databases. */
    opened: readonly string[];
    /** Open (if needed) and activate a database of the connection. */
    openDatabase: (name: string) => void;
    /** Close an opened database and its sessions. */
    closeDatabase: (name: string) => void;
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
  if (!profile) return 'Choose a connection to browse its objects.';
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
  const database = profileId ? connections.database : '';
  const { chosenSchema, ...navigation } = useNavigation(profileId, database);
  const prefix = `${profileId ?? ''}
${String(connected?.generation ?? '')}
`;
  const reads = useReads(resources, prefix, profileId, {
    home: connected?.server.database ?? '',
    database,
  });
  const columnTree = useColumns(
    resources,
    `${prefix}${database}
`,
    {
      profileId,
      database,
    },
  );
  const schema = pickSchema(
    reads.schemas,
    chosenSchema,
    navigation.systemSchemas,
  );
  const objectsKey = (group: ObjectGroup): string =>
    `${prefix}${database}
objects
${schema}
${group.id}`;
  return {
    ...navigation,
    database,
    opened: profileId ? connections.databases.map(({ name }) => name) : [],
    openDatabase: connections.openDatabase,
    closeDatabase: connections.closeDatabase,
    offline: offlineReason(connections),
    ...reads,
    ...columnTree,
    schema,
    objects: (group) => resources.get(objectsKey(group)),
    loadObjects: (group) => {
      if (!profileId || !schema) return;
      resources.load(objectsKey(group), () =>
        listObjects(profileId, database, schema, group.kinds),
      );
    },
    refresh: () => {
      resources.invalidate(`${prefix}${database}
`);
      resources.invalidate(`${prefix}databases`);
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

/**
 * The server's databases load through the home database as soon as the
 * connection is up; schemas once a database is active; search on demand.
 */
function useReads(
  resources: Resources,
  prefix: string,
  profileId: string | null,
  { home, database }: { home: string; database: string },
): Pick<Catalog, 'databases' | 'schemas' | 'search' | 'loadSearch'> {
  const { load } = resources;
  const scoped = `${prefix}${database}
`;
  const databases = resources.get<DatabaseDto[]>(`${prefix}databases`);
  const schemas = resources.get<SchemaDto[]>(`${scoped}schemas`);
  useEffect(() => {
    if (!profileId || !home) return;
    load(`${prefix}databases`, () => listDatabases(profileId, home));
  }, [load, prefix, profileId, home, databases.status]);
  useEffect(() => {
    if (!profileId || !database) return;
    load(`${scoped}schemas`, () => listSchemas(profileId, database));
  }, [load, scoped, profileId, database, schemas.status]);
  return {
    databases,
    schemas,
    search: resources.get<ObjectDto[]>(`${scoped}search`),
    loadSearch: () => {
      if (!profileId || !database) return;
      load(`${scoped}search`, () => searchCatalog(profileId, database));
    },
  };
}
