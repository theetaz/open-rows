// SPDX-License-Identifier: Apache-2.0
import type {
  CatalogColumn,
  CatalogObject,
  CatalogSchema,
  CompletionCatalog,
} from '@openrows/editor';

/** The schema benchmark's size (NFR-PERF-08): 50 000 user objects. */
export const CATALOG_OBJECTS = 50_000;
const SCHEMAS = ['public', 'sales', 'billing', 'audit', 'crm'] as const;
const STEMS = ['orders', 'customers', 'invoices', 'events', 'products'];
const KINDS: CatalogObject['kind'][] = [
  'table',
  'table',
  'table',
  'view',
  'function',
];
const COLUMNS = 40;

/** Deterministic object names such as `orders_00042`, spread over five schemas. */
function objects(): CatalogObject[] {
  const perSchema = CATALOG_OBJECTS / SCHEMAS.length;
  return SCHEMAS.flatMap((schema) =>
    Array.from({ length: perSchema }, (_, index): CatalogObject => {
      const stem = STEMS[index % STEMS.length] ?? 'orders';
      const name = `${stem}_${String(index).padStart(5, '0')}`;
      return { schema, name, kind: KINDS[index % KINDS.length] ?? 'table' };
    }),
  );
}

function columns(): CatalogColumn[] {
  return Array.from({ length: COLUMNS }, (_, index) => ({
    name: index === 0 ? 'id' : `attribute_${String(index)}`,
    dataType: index === 0 ? 'bigint' : index % 3 ? 'text' : 'numeric',
  }));
}

/**
 * A warm in-memory catalog, like the app's per-connection cache after its
 * first reads: every method resolves at once with the same arrays.
 */
export function syntheticCatalog(): CompletionCatalog {
  const all = objects();
  const schemas: CatalogSchema[] = SCHEMAS.map((name) => ({
    name,
    system: false,
  }));
  const bySchema = new Map<string, CatalogObject[]>(
    SCHEMAS.map((schema) => [schema, all.filter((o) => o.schema === schema)]),
  );
  const relation = columns();
  return {
    schemas: () => Promise.resolve(schemas),
    objects: () => Promise.resolve(all),
    objectsIn: (schema) => Promise.resolve(bySchema.get(schema) ?? []),
    columns: () => Promise.resolve(relation),
  };
}
