// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type {
  CatalogReply,
  CatalogSearchReply,
  CatalogSearchRequest,
  DatabaseDto,
  ObjectDto,
  ObjectKindDto,
  ObjectIdentityDto,
  ObjectsReply,
  SchemaDto,
} from '@openrows/ipc';

const object = (
  database: string,
  schema: string,
  name: string,
  kind: ObjectKindDto,
  extra: Partial<ObjectDto> = {},
): ObjectDto & { database: string } => ({
  database,
  schema,
  name,
  kind,
  arguments: null,
  rowEstimate: null,
  comment: null,
  ...extra,
});

/** Synthetic server catalog: three user databases and one system database. */
const catalogDatabases: DatabaseDto[] = [
  { name: 'analytics_scratch', system: false, comment: null },
  { name: 'orders', system: false, comment: null },
  { name: 'orders_archive', system: false, comment: null },
  { name: 'postgres', system: true, comment: null },
];

const schemas: Record<string, SchemaDto[]> = {
  orders: [
    { name: 'archive', system: false, comment: null },
    { name: 'public', system: false, comment: null },
    { name: 'pg_catalog', system: true, comment: null },
  ],
};

const objects = [
  object('orders', 'public', 'customers', 'table', { rowEstimate: 412_000 }),
  object('orders', 'public', 'order_items', 'table', {
    rowEstimate: 9_100_000,
  }),
  object('orders', 'public', 'orders', 'table', { rowEstimate: 2_400_000 }),
  object('orders', 'public', 'active_customers', 'view'),
  object('orders', 'public', 'order_summary', 'view'),
  object('orders', 'public', 'daily_revenue', 'materializedView', {
    rowEstimate: 365,
  }),
  object('orders', 'public', 'calc_tax', 'function', {
    arguments: 'total numeric',
  }),
  object('orders', 'public', 'refresh_totals', 'procedure', {
    arguments: '',
  }),
  object('orders', 'public', 'orders_id_seq', 'sequence'),
  object('orders', 'archive', 'session_notes', 'table', { rowEstimate: 16 }),
  object('orders_archive', 'public', 'orders_2024', 'table', {
    rowEstimate: 820_000,
  }),
  object('orders_archive', 'public', 'orders_2024_id_seq', 'sequence'),
];

const strip = (item: ObjectDto & { database: string }): ObjectDto => ({
  schema: item.schema,
  name: item.name,
  kind: item.kind,
  arguments: item.arguments,
  rowEstimate: item.rowEstimate,
  comment: item.comment,
});

/** Fresh schema-cache reply around synthetic data. */
const fresh = <T>(data: T): Promise<CatalogReply<T>> =>
  Promise.resolve({ data, stale: false });

/** Substring, or loose in-order letters, like the Rust name index. */
const matches = (name: string, query: string): boolean => {
  const text = name.toLowerCase();
  const needle = query.toLowerCase();
  if (text.includes(needle)) return true;
  let from = 0;
  for (const letter of needle) {
    from = text.indexOf(letter, from) + 1;
    if (!from) return false;
  }
  return true;
};

function search(
  database: string,
  request: CatalogSearchRequest,
): Promise<CatalogSearchReply> {
  const found = objects.filter(
    (item) =>
      item.database === database &&
      (request.schema === null || item.schema === request.schema) &&
      matches(item.name, request.query),
  );
  return Promise.resolve({
    objects: found.slice(0, request.limit ?? found.length).map(strip),
    total: found.length,
    stale: false,
  });
}

/** Catalog commands of `@openrows/ipc` over the synthetic catalog. */
export function catalogStub(): Record<string, unknown> {
  return {
    catalogCapabilities: vi.fn(() =>
      Promise.resolve({
        kinds: [
          'table',
          'foreignTable',
          'view',
          'materializedView',
          'function',
          'procedure',
          'sequence',
          'type',
        ],
        extensions: true,
        roles: true,
      }),
    ),
    listRoles: vi.fn(() => Promise.resolve([])),
    listExtensions: vi.fn(() => Promise.resolve([])),
    roleGrants: vi.fn(() => Promise.resolve([])),
    inspectObject: vi.fn(
      (_profile: string, _database: string, object: ObjectIdentityDto) =>
        Promise.resolve(
          object.kind === 'sequence'
            ? {
                attributes: [{ name: 'Start', value: '17' }],
                sections: [],
                definition: null,
              }
            : {
                attributes: [],
                sections: [],
                definition: `CREATE ${object.kind.toUpperCase()} ${object.schema}.${object.name} AS SELECT 1;`,
              },
        ),
    ),
    listDatabases: vi.fn(() => fresh(catalogDatabases)),
    listSchemas: vi.fn((_profile: string, database: string) =>
      fresh(
        schemas[database] ?? [{ name: 'public', system: false, comment: null }],
      ),
    ),
    listObjects: vi.fn(
      (
        _profile: string,
        database: string,
        section: {
          schema: string;
          kinds: ObjectKindDto[];
          limit?: number | null;
        },
      ): Promise<ObjectsReply> => {
        const found = objects
          .filter(
            (item) =>
              item.database === database &&
              item.schema === section.schema &&
              section.kinds.includes(item.kind),
          )
          .map(strip);
        return Promise.resolve({
          data: found.slice(0, section.limit ?? found.length),
          total: found.length,
          stale: false,
        });
      },
    ),
    checkCatalog: vi.fn(() => Promise.resolve(false)),
    searchCatalog: vi.fn(
      (_profile: string, database: string, request: CatalogSearchRequest) =>
        search(database, request),
    ),
    invalidateCatalog: vi.fn(() => Promise.resolve()),
    onCatalogChanged: vi.fn(() => Promise.resolve(() => undefined)),
  };
}
