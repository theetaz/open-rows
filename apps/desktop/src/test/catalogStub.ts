// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type {
  DatabaseDto,
  ObjectDto,
  ObjectKindDto,
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
];

const strip = (item: ObjectDto & { database: string }): ObjectDto => ({
  schema: item.schema,
  name: item.name,
  kind: item.kind,
  arguments: item.arguments,
  rowEstimate: item.rowEstimate,
  comment: item.comment,
});

/** Catalog commands of `@openrows/ipc` over the synthetic catalog. */
export function catalogStub(): Record<string, unknown> {
  return {
    listDatabases: vi.fn(() => Promise.resolve(catalogDatabases)),
    listSchemas: vi.fn((_profile: string, database: string) =>
      Promise.resolve(
        schemas[database] ?? [{ name: 'public', system: false, comment: null }],
      ),
    ),
    listObjects: vi.fn(
      (
        _profile: string,
        database: string,
        schema: string,
        kinds: ObjectKindDto[],
      ) =>
        Promise.resolve(
          objects
            .filter(
              (item) =>
                item.database === database &&
                item.schema === schema &&
                kinds.includes(item.kind),
            )
            .map(strip),
        ),
    ),
    searchCatalog: vi.fn((_profile: string, database: string) =>
      Promise.resolve(
        objects.filter((item) => item.database === database).map(strip),
      ),
    ),
  };
}
