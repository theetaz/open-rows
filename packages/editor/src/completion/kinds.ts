// SPDX-License-Identifier: Apache-2.0
import { Facet } from '@codemirror/state';
import type { Candidate } from './rank';

/** Which suggestion kinds completion offers (keywords and functions always). */
type CompletionKinds = {
  schemas: boolean;
  /** Tables and views. */
  tables: boolean;
  columns: boolean;
  snippets: boolean;
};

/** Every kind on: the default. */
const allKinds: CompletionKinds = {
  schemas: true,
  tables: true,
  columns: true,
  snippets: true,
};

/** The suggestion kinds in effect; the last provided value wins. */
export const completionKinds = Facet.define<CompletionKinds, CompletionKinds>({
  combine: (values) => values[values.length - 1] ?? allKinds,
});

/** Whether `kinds` lets a candidate of this kind through. */
export function offered(candidate: Candidate, kinds: CompletionKinds): boolean {
  switch (candidate.kind) {
    case 'schema':
      return kinds.schemas;
    case 'table':
    case 'view':
      return kinds.tables;
    case 'column':
      return kinds.columns;
    default:
      return true;
  }
}
