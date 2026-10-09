// SPDX-License-Identifier: Apache-2.0
import type { ObjectDto, ObjectKindDto, SchemaDto } from '@openrows/ipc';

/** One disclosure section of the object tree. */
export type ObjectGroup = {
  id: string;
  label: string;
  kinds: ObjectKindDto[];
};

/** Tree sections in display order; each loads its kinds on first expand. */
export const objectGroups: readonly ObjectGroup[] = [
  { id: 'tables', label: 'Tables', kinds: ['table', 'foreignTable'] },
  { id: 'views', label: 'Views', kinds: ['view'] },
  {
    id: 'materializedViews',
    label: 'Materialized views',
    kinds: ['materializedView'],
  },
  { id: 'functions', label: 'Functions', kinds: ['function'] },
  { id: 'procedures', label: 'Procedures', kinds: ['procedure'] },
  { id: 'sequences', label: 'Sequences', kinds: ['sequence'] },
  { id: 'types', label: 'Types', kinds: ['type'] },
];

const kinds: Record<ObjectKindDto, { label: string; glyph: string }> = {
  table: { label: 'Table', glyph: '▦' },
  foreignTable: { label: 'Foreign table', glyph: '▦' },
  view: { label: 'View', glyph: '▤' },
  materializedView: { label: 'Materialized view', glyph: '▥' },
  function: { label: 'Function', glyph: 'ƒ' },
  procedure: { label: 'Procedure', glyph: 'ƒ' },
  sequence: { label: 'Sequence', glyph: '≡' },
  type: { label: 'Type', glyph: 'T' },
};

/** Display name and glyph of an object kind. */
export function kindInfo(kind: ObjectKindDto): {
  label: string;
  glyph: string;
} {
  return kinds[kind];
}

/** Relations whose tree rows expand to list their columns. */
export function hasColumns(kind: ObjectKindDto): boolean {
  return (
    kind === 'table' ||
    kind === 'foreignTable' ||
    kind === 'view' ||
    kind === 'materializedView'
  );
}

/** Tree selection key of a listed object (unique within a database). */
export function nodeKey(object: ObjectDto): string {
  return `${object.schema}\n${object.kind}\n${objectLabel(object)}`;
}

/** Tree selection key of one column row under its object's key. */
export function columnKey(objectKey: string, column: string): string {
  return `${objectKey}\n${column}`;
}

/** A real object path handed to tabs: `[database, schema, name]` plus kind. */
export type ObjectRef = {
  database: string;
  schema: string;
  name: string;
  kind: ObjectKindDto;
  /** Routine identity arguments; distinguishes overloads. */
  arguments: string | null;
};

/** Name as listed: routines show their identity arguments. */
export function objectLabel(
  object: Pick<ObjectDto, 'name' | 'arguments'>,
): string {
  return object.arguments === null
    ? object.name
    : `${object.name}(${object.arguments})`;
}

/** Path for a listed object in `database`. */
export function objectRef(database: string, object: ObjectDto): ObjectRef {
  return {
    database,
    schema: object.schema,
    name: object.name,
    kind: object.kind,
    arguments: object.arguments,
  };
}

/** Same object (routine overloads differ by arguments). */
export function sameObject(a: ObjectRef, b: ObjectRef): boolean {
  return (
    a.database === b.database &&
    a.schema === b.schema &&
    a.name === b.name &&
    a.kind === b.kind &&
    a.arguments === b.arguments
  );
}

const units: readonly [number, string][] = [
  [1e9, 'B'],
  [1e6, 'M'],
  [1e3, 'K'],
];

/** Compact planner estimate, e.g. `~2.4M`; `undefined` when unknown. */
export function formatEstimate(rows: number | null): string | undefined {
  if (rows === null) return undefined;
  for (const [size, unit] of units) {
    if (rows >= size) {
      const value = rows / size;
      return `~${value < 10 ? value.toFixed(1).replace(/\.0$/, '') : String(Math.round(value))}${unit}`;
    }
  }
  return `~${String(Math.round(rows))}`;
}

/** `public` when present, else the first user schema, else the first schema. */
export function defaultSchema(schemas: readonly SchemaDto[]): string {
  const user = schemas.filter((schema) => !schema.system);
  return (
    user.find((schema) => schema.name === 'public')?.name ??
    user[0]?.name ??
    schemas[0]?.name ??
    ''
  );
}

/** All substring matches, including repeated occurrences, for highlighting. */
export function substringIndices(text: string, query: string): number[] {
  if (!query) return [];
  const source = text.toLowerCase();
  const needle = query.toLowerCase();
  const indices: number[] = [];
  let index = source.indexOf(needle);
  while (index >= 0) {
    for (let offset = 0; offset < needle.length; offset++)
      indices.push(index + offset);
    index = source.indexOf(needle, index + needle.length);
  }
  return indices;
}

/** Sections claimed by the driver; unsupported kinds have no disclosure or read. */
export function supportedGroups(
  supported: readonly ObjectKindDto[],
): ObjectGroup[] {
  return objectGroups
    .map((group) => ({
      ...group,
      kinds: group.kinds.filter((kind) => supported.includes(kind)),
    }))
    .filter((group) => group.kinds.length > 0);
}
