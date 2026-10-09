// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect } from '../dialects';
import {
  builtInDetail,
  builtInFunctions,
  dialectKeywords,
  keywordPriority,
} from './builtins';
import { currentSchema } from './catalog';
import type {
  CatalogObject,
  CatalogSchema,
  CompletionCatalog,
} from './catalog';
import type { TableRef } from './context';
import type { Candidate } from './rank';

/** What candidate collection needs besides the statement context. */
export type CandidateInput = {
  dialect: EditorDialect;
  catalog: CompletionCatalog | null;
  /** Insert keywords in upper case. */
  upperKeywords: boolean;
};

/** Catalog reads never fail completion; a failed read offers nothing. */
export async function safely<T>(
  read: (() => Promise<readonly T[]>) | undefined,
): Promise<readonly T[]> {
  if (!read) return [];
  try {
    return await read();
  } catch {
    return [];
  }
}

/** Quote a name the dialect would otherwise fold or reject. */
export function quoteName(name: string, dialect: EditorDialect): string {
  const plain =
    dialect === 'postgres' ? /^[a-z_][a-z0-9_$]*$/ : /^[A-Za-z_][\w$]*$/;
  if (plain.test(name)) return name;
  if (dialect === 'sqlserver') return `[${name.replaceAll(']', ']]')}]`;
  const quote = dialect === 'mysql' || dialect === 'mariadb' ? '`' : '"';
  return quote + name.replaceAll(quote, quote + quote) + quote;
}

function named(label: string, dialect: EditorDialect): { apply?: string } {
  const quoted = quoteName(label, dialect);
  return quoted === label ? {} : { apply: quoted };
}

export function keywords(input: CandidateInput): Candidate[] {
  return dialectKeywords(input.dialect).map((keyword) => ({
    label: input.upperKeywords ? keyword.toUpperCase() : keyword,
    kind: 'keyword',
    detail: 'keyword',
    schemaRank: 0,
    priority: keywordPriority(keyword),
  }));
}

/** Rank of a schema: current 0, other user schemas 1, system schemas 2. */
export function schemaRanker(
  schemas: readonly CatalogSchema[],
): (schema: string) => number {
  const current = currentSchema(schemas);
  const system = new Set(schemas.filter((s) => s.system).map((s) => s.name));
  return (schema) => {
    if (schema === current) return 0;
    return system.has(schema) ? 2 : 1;
  };
}

export function objectCandidates(
  objects: readonly CatalogObject[],
  rank: (schema: string) => number,
  dialect: EditorDialect,
): Candidate[] {
  return objects.map((object) => ({
    label: object.name,
    kind: object.kind,
    detail: object.schema,
    schemaRank: rank(object.schema),
    priority: 0,
    ...named(object.name, dialect),
  }));
}

export function builtIns(dialect: EditorDialect): Candidate[] {
  return builtInFunctions(dialect).map((name) => ({
    label: name,
    kind: 'function',
    detail: builtInDetail(dialect),
    schemaRank: 2,
    priority: 0,
  }));
}

/** Everything the catalog index offers: objects of all user schemas and schemas. */
export async function catalogCandidates(
  input: CandidateInput,
): Promise<Candidate[]> {
  const { catalog, dialect } = input;
  const [schemas, objects] = await Promise.all([
    safely(catalog ? () => catalog.schemas() : undefined),
    safely(catalog ? () => catalog.objects() : undefined),
  ]);
  const rank = schemaRanker(schemas);
  const schemaRows: Candidate[] = schemas.map((schema) => ({
    label: schema.name,
    kind: 'schema',
    detail: 'schema',
    schemaRank: rank(schema.name),
    priority: 0,
    ...named(schema.name, dialect),
  }));
  return [...objectCandidates(objects, rank, dialect), ...schemaRows];
}

/** The schema and name a reference resolves to in the catalog index. */
async function resolve(
  ref: Pick<TableRef, 'schema' | 'name'>,
  catalog: CompletionCatalog,
): Promise<{ schema: string; name: string }> {
  if (ref.schema) return { schema: ref.schema, name: ref.name };
  const [schemas, objects] = await Promise.all([
    safely(() => catalog.schemas()),
    safely(() => catalog.objects()),
  ]);
  const current = currentSchema(schemas);
  const lower = ref.name.toLowerCase();
  const matches = objects.filter(
    (object) =>
      object.kind !== 'function' && object.name.toLowerCase() === lower,
  );
  const match =
    matches.find((object) => object.schema === current) ?? matches[0];
  return match ?? { schema: current, name: ref.name };
}

/** Columns of the given relations in ordinal order, detailed with their table name. */
export async function columnCandidates(
  refs: readonly Pick<TableRef, 'schema' | 'name'>[],
  input: CandidateInput,
): Promise<Candidate[]> {
  const { catalog, dialect } = input;
  if (!catalog) return [];
  const lists = await Promise.all(
    refs.map(async (ref) => {
      const { schema, name } = await resolve(ref, catalog);
      const columns = await safely(() => catalog.columns(schema, name));
      return columns.map((column, ordinal): Candidate => ({
        label: column.name,
        kind: 'column',
        detail: name,
        schemaRank: 0,
        priority: ordinal,
        ...named(column.name, dialect),
      }));
    }),
  );
  return lists.flat();
}
