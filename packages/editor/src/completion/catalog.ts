// SPDX-License-Identifier: Apache-2.0

/** Kinds of completion rows; each has its own icon. */
export type CompletionKind =
  'keyword' | 'table' | 'view' | 'function' | 'column' | 'schema' | 'snippet';

/** A schema of the edited database. */
export type CatalogSchema = { name: string; system: boolean };

/** A table, view or routine of one schema. */
export type CatalogObject = {
  schema: string;
  name: string;
  kind: 'table' | 'view' | 'function';
};

/** A relation column in ordinal order. */
export type CatalogColumn = { name: string; dataType: string };

/**
 * Schema data for completion, read lazily and cached by the application per
 * connection and database. Every method may reject; completion then falls
 * back to what it has (keywords at least).
 */
export interface CompletionCatalog {
  /** Schemas of the database, system schemas flagged. */
  readonly schemas: () => Promise<readonly CatalogSchema[]>;
  /** Tables, views and routines of every user schema (a bounded index). */
  readonly objects: () => Promise<readonly CatalogObject[]>;
  /** Objects of one schema, system schemas included. */
  readonly objectsIn: (schema: string) => Promise<readonly CatalogObject[]>;
  /** Columns of one relation; empty when it does not exist. */
  readonly columns: (
    schema: string,
    table: string,
  ) => Promise<readonly CatalogColumn[]>;
}

/** `public` when present, else the first user schema, else the first schema. */
export function currentSchema(schemas: readonly CatalogSchema[]): string {
  const user = schemas.filter((schema) => !schema.system);
  return (
    user.find((schema) => schema.name === 'public')?.name ??
    user[0]?.name ??
    schemas[0]?.name ??
    ''
  );
}
