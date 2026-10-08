// SPDX-License-Identifier: Apache-2.0
import {
  builtIns,
  catalogCandidates,
  columnCandidates,
  keywords,
  objectCandidates,
  safely,
  schemaRanker,
} from './candidates';
import type { CandidateInput } from './candidates';
import type { StatementContext, TableRef } from './context';
import type { Candidate } from './rank';

/**
 * `alias.` and `table.` → that relation's columns; `schema.` → the schema's
 * objects; `schema.table.` → its columns.
 */
async function memberCandidates(
  path: readonly string[],
  references: readonly TableRef[],
  input: CandidateInput,
): Promise<Candidate[]> {
  const { catalog } = input;
  const last = path[path.length - 1];
  if (!catalog || last === undefined) return [];
  if (path.length > 1)
    return columnCandidates(
      [{ schema: path[path.length - 2] ?? null, name: last }],
      input,
    );
  const lower = last.toLowerCase();
  const ref =
    references.find((item) => item.alias?.toLowerCase() === lower) ??
    references.find((item) => item.name.toLowerCase() === lower);
  if (ref) return columnCandidates([ref], input);
  const schemas = await safely(() => catalog.schemas());
  const schema = schemas.find((item) => item.name.toLowerCase() === lower);
  if (!schema) return columnCandidates([{ schema: null, name: last }], input);
  const objects = await safely(() => catalog.objectsIn(schema.name));
  return objectCandidates(objects, schemaRanker(schemas), input.dialect);
}

/** Candidate rows for the scope at the cursor; ranking filters by kind. */
export async function gatherCandidates(
  context: StatementContext,
  input: CandidateInput,
): Promise<Candidate[]> {
  const { scope, references } = context;
  switch (scope.kind) {
    case 'start':
      return keywords(input);
    case 'member':
      return memberCandidates(scope.path, references, input);
    case 'relation':
      return [
        ...(await catalogCandidates(input)),
        ...builtIns(input.dialect),
        ...keywords(input),
      ];
    case 'expression': {
      const [columns, objects] = await Promise.all([
        columnCandidates(references, input),
        catalogCandidates(input),
      ]);
      return [
        ...columns,
        ...builtIns(input.dialect),
        ...keywords(input),
        ...objects,
      ];
    }
  }
}
