// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  invalidateCatalog,
  listDatabases,
  listObjects,
  listSchemas,
} from '@openrows/ipc';
import type {
  DatabaseDto,
  EngineDto,
  ObjectDto,
  SchemaDto,
} from '@openrows/ipc';
import type { Connections } from '../connections';
import { defaultSchema, objectGroups } from './catalog';
import type { ObjectRef } from './catalog';
import { catalogAvailability } from './objectAvailability';
import type { ObjectGroup } from './catalog';
import { useCatalogChanges, useCatalogIndex } from './useCatalogIndex';
import type { CatalogIndex } from './useCatalogIndex';
import { useColumns } from './useColumns';
import { useInspectionCatalog } from './useInspectionCatalog';
import type { InspectionCatalog } from './useInspectionCatalog';
import type { ColumnTree } from './useColumns';
import { useNavigation } from './useNavigation';
import type { NavigationState } from './useNavigation';
import { isRefreshing, useResources } from './useResources';
import { useSchemaFingerprint } from './useSchemaFingerprint';
import type { Resource, Resources } from './useResources';

/** The active connection's catalog, served by the schema cache (M1-16). */
export type Catalog = Omit<NavigationState, 'chosenSchema'> &
  ColumnTree &
  CatalogIndex & {
    /** Why there is nothing to browse, or `null` when connected. */
    offline: string | null;
    profileId: string;
    engine: EngineDto;
    /** Availability from complete normal section reads, null until verified. */
    available: (object: ObjectRef) => boolean | null;
    inspection: InspectionCatalog;
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
    /** The first `limit` objects of a section and its total. */
    objects: (
      group: ObjectGroup,
      limit?: number,
      schema?: string,
    ) => Resource<ObjectPage>;
    loadObjects: (group: ObjectGroup, limit?: number, schema?: string) => void;
    /** Something shown came from a snapshot and is being read again. */
    refreshing: boolean;
    /** Changes on every refresh and after DDL; completion re-reads on it. */
    revision: number;
    /** Read everything shown from the active connection again, keeping it on screen. */
    refresh: () => void;
  };

/** Objects a tree section shows at first and adds per "Show more". */
export const OBJECT_PAGE = 5000;

/** The first objects of a section and how many it holds in all. */
export type ObjectPage = { objects: ObjectDto[]; total: number };

const offlineReason = (connections: Connections): string | null => {
  const profile = connections.active;
  if (!profile) return 'Choose a connection to browse its objects.';
  if (profile.engine === 'duckdb')
    return 'DuckDB catalog browsing is not available yet. Open a query tab to run SQL.';
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
  const profileId =
    connected && connections.active?.engine !== 'duckdb'
      ? (connections.active?.id ?? null)
      : null;
  const database = connected ? connections.database : '';
  const { chosenSchema, ...navigation } = useNavigation(profileId, database);
  const prefix = `${profileId ?? ''}\n${String(connected?.generation ?? '')}\n`;
  const scoped = `${prefix}${database}\n`;
  const target = { profileId, database };
  const reads = useReads(resources, prefix, profileId, {
    home: connected?.server.database ?? '',
    database,
  });
  const columnTree = useColumns(resources, scoped, target);
  const inspection = useInspectionCatalog(resources, scoped, target);
  const index = useCatalogIndex(resources, scoped, target);
  const [revision, refresh] = useRefresh(resources, profileId, prefix);
  useSchemaFingerprint(profileId, database);
  const schema = pickSchema(
    reads.schemas,
    chosenSchema,
    navigation.systemSchemas,
  );
  const objectsKey = (
    group: ObjectGroup,
    limit: number,
    selectedSchema = schema,
  ): string =>
    `${scoped}objects\n${selectedSchema}\n${group.id}\n${String(limit)}`;
  const objects = (
    group: ObjectGroup,
    limit = OBJECT_PAGE,
    selectedSchema = schema,
  ): Resource<ObjectPage> =>
    resources.get(objectsKey(group, limit, selectedSchema));
  const available = useAvailability(resources, scoped, reads.schemas);
  return {
    ...navigation,
    ...catalogContext(connections),
    available,
    inspection,
    database,
    opened: openedDatabases(connections, profileId),
    openDatabase: connections.openDatabase,
    closeDatabase: connections.closeDatabase,
    offline: offlineReason(connections),
    ...reads,
    ...columnTree,
    ...index,
    schema,
    objects,
    loadObjects: (group, limit = OBJECT_PAGE, selectedSchema = schema) => {
      if (!profileId || !selectedSchema) return;
      const section = { schema: selectedSchema, kinds: group.kinds, limit };
      resources.load(objectsKey(group, limit, selectedSchema), (fresh) =>
        listObjects(profileId, database, section, fresh).then((reply) => ({
          data: { objects: reply.data, total: reply.total },
          stale: reply.stale,
        })),
      );
    },
    refreshing: [
      reads.databases,
      reads.schemas,
      ...objectGroups.map((group) => objects(group)),
    ].some(isRefreshing),
    revision,
    refresh,
  };
}

/**
 * Manual refresh (⌘R) and refresh after DDL: the Rust cache marks the
 * connection stale, then every loaded read is read again in place.
 */
function useRefresh(
  resources: Resources,
  profileId: string | null,
  prefix: string,
): [number, () => void] {
  const [revision, setRevision] = useState(0);
  const { revalidate } = resources;
  const reread = useCallback(() => {
    revalidate(prefix);
    setRevision((count) => count + 1);
  }, [revalidate, prefix]);
  useCatalogChanges(profileId, reread);
  return [
    revision,
    () => {
      if (profileId) invalidateCatalog(profileId).catch(() => undefined);
      reread();
    },
  ];
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
 * connection is up; schemas once a database is active.
 */
function useReads(
  resources: Resources,
  prefix: string,
  profileId: string | null,
  { home, database }: { home: string; database: string },
): Pick<Catalog, 'databases' | 'schemas'> {
  const { load } = resources;
  const scoped = `${prefix}${database}\n`;
  const databases = resources.get<DatabaseDto[]>(`${prefix}databases`);
  const schemas = resources.get<SchemaDto[]>(`${scoped}schemas`);
  useEffect(() => {
    if (!profileId || !home) return;
    load(`${prefix}databases`, (fresh) =>
      listDatabases(profileId, home, fresh),
    );
  }, [load, prefix, profileId, home, databases.status]);
  useEffect(() => {
    if (!profileId || !database) return;
    load(`${scoped}schemas`, (fresh) =>
      listSchemas(profileId, database, fresh),
    );
  }, [load, scoped, profileId, database, schemas.status]);
  return { databases, schemas };
}

/** Persisted UI scope also exists while disconnected, unlike the live read target. */
function catalogContext(connections: Connections): {
  profileId: string;
  engine: EngineDto;
} {
  return {
    profileId: connections.active?.id ?? '',
    engine: connections.active?.engine ?? 'postgres',
  };
}
function openedDatabases(
  connections: Connections,
  profileId: string | null,
): string[] {
  return profileId ? connections.databases.map(({ name }) => name) : [];
}

function useAvailability(
  resources: Resources,
  scoped: string,
  schemas: Resource<SchemaDto[]>,
): Catalog['available'] {
  const { entries } = resources;
  const prefix = `${scoped}objects\n`;
  return useMemo(
    () =>
      catalogAvailability({
        schemas,
        pages: entries<ObjectPage>(prefix).map(([key, value]) => [
          key.slice(prefix.length),
          value,
        ]),
      }),
    [entries, prefix, schemas],
  );
}
