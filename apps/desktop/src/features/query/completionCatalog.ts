// SPDX-License-Identifier: Apache-2.0
import {
  listObjects,
  listSchemas,
  searchCatalog,
  tableStructure,
} from '@openrows/ipc';
import type { ObjectDto, ObjectKindDto } from '@openrows/ipc';
import type { CatalogObject, CompletionCatalog } from '@openrows/editor';

/** Object kinds `schema.` completion lists. */
const KINDS: ObjectKindDto[] = [
  'table',
  'view',
  'materializedView',
  'foreignTable',
  'function',
  'procedure',
];

const KIND_OF: Partial<Record<ObjectKindDto, CatalogObject['kind']>> = {
  table: 'table',
  foreignTable: 'table',
  view: 'view',
  materializedView: 'view',
  function: 'function',
  procedure: 'function',
};

function toObjects(objects: readonly ObjectDto[]): CatalogObject[] {
  return objects.flatMap((object) => {
    const kind = KIND_OF[object.kind];
    return kind ? [{ schema: object.schema, name: object.name, kind }] : [];
  });
}

/**
 * Reads of one connection database, each made at most once per revision:
 * failures are kept too, so a missing table is not re-read per keystroke.
 */
function readOnce(profileId: string, database: string): CompletionCatalog {
  const reads = new Map<string, Promise<unknown>>();
  const once = <T>(key: string, read: () => Promise<T>): Promise<T> => {
    const cached = reads.get(key) as Promise<T> | undefined;
    if (cached) return cached;
    const started = read();
    started.catch(() => undefined);
    reads.set(key, started);
    return started;
  };
  return {
    schemas: () =>
      once('schemas', async () =>
        (await listSchemas(profileId, database)).map(({ name, system }) => ({
          name,
          system,
        })),
      ),
    objects: () =>
      once('objects', async () =>
        toObjects(await searchCatalog(profileId, database)),
      ),
    objectsIn: (schema) =>
      once(`objects\n${schema}`, async () =>
        toObjects(await listObjects(profileId, database, schema, KINDS)),
      ),
    columns: (schema, table) =>
      once(`columns\n${schema}\n${table}`, async () => {
        const target = { profileId, database, schema, table };
        const { columns } = await tableStructure(target);
        return columns.map(({ name, dataType }) => ({ name, dataType }));
      }),
  };
}

const caches = new Map<
  string,
  { revision: string; catalog: CompletionCatalog }
>();

/**
 * The completion catalog of a connection database, shared by its tabs.
 * `revision` changes on reconnect and on schema refresh, which drops every
 * cached read.
 */
export function completionCatalog(target: {
  profileId: string;
  database: string;
  revision: string;
}): CompletionCatalog {
  const key = `${target.profileId}\n${target.database}`;
  const cached = caches.get(key);
  if (cached?.revision === target.revision) return cached.catalog;
  const catalog = readOnce(target.profileId, target.database);
  caches.set(key, { revision: target.revision, catalog });
  return catalog;
}

const offline: CompletionCatalog = {
  schemas: () => Promise.resolve([]),
  objects: () => Promise.resolve([]),
  objectsIn: () => Promise.resolve([]),
  columns: () => Promise.resolve([]),
};

/**
 * A stable catalog for a mounted editor that always reads through the
 * tab's current catalog (none while disconnected).
 */
export function currentCatalog(
  current: () => CompletionCatalog | null,
): CompletionCatalog {
  const pick = (): CompletionCatalog => current() ?? offline;
  return {
    schemas: () => pick().schemas(),
    objects: () => pick().objects(),
    objectsIn: (schema) => pick().objectsIn(schema),
    columns: (schema, table) => pick().columns(schema, table),
  };
}
