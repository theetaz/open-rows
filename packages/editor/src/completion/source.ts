// SPDX-License-Identifier: Apache-2.0
import type {
  Completion,
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import { Facet } from '@codemirror/state';
import type { EditorState } from '@codemirror/state';
import type { EditorDialect } from '../dialects';
import { editingRange } from '../statement';
import type { CompletionCatalog } from './catalog';
import { analyzeStatement } from './context';
import { gatherCandidates } from './gather';
import { completionKinds, offered } from './kinds';
import { tokenize } from './lexer';
import { rankCandidates } from './rank';
import type { Candidate } from './rank';

/** Whether accepted keywords are inserted in upper case (default on). */
export const upperKeywords = Facet.define<boolean, boolean>({
  combine: (values) => values[values.length - 1] ?? true,
});

const WORD = /[A-Za-z_\u0080-￿][\w$\u0080-￿]*/;
const LITERALS = new Set([
  'String',
  'LineComment',
  'BlockComment',
  'QuotedIdentifier',
]);

function inLiteral(state: EditorState, pos: number): boolean {
  return LITERALS.has(syntaxTree(state).resolveInner(pos, -1).name);
}

function toCompletion(candidate: Candidate): Completion {
  return {
    label: candidate.label,
    type: candidate.kind,
    detail: candidate.detail,
    ...(candidate.apply ? { apply: candidate.apply } : {}),
  };
}

/**
 * Context-aware SQL completion: keywords, schema objects and columns of the
 * relations the statement references, ranked for the typed prefix. Catalog
 * reads are async and cached by the catalog, so typing never waits on them.
 */
export function sqlCompletionSource(
  dialect: EditorDialect,
  catalog: CompletionCatalog | null,
): CompletionSource {
  return async (
    context: CompletionContext,
  ): Promise<CompletionResult | null> => {
    const { state, pos } = context;
    const word = context.matchBefore(WORD);
    const from = word?.from ?? pos;
    const typed = word?.text ?? '';
    const afterDot = state.sliceDoc(from - 1, from) === '.';
    if (!context.explicit && !typed && !afterDot) return null;
    if (inLiteral(state, pos)) return null;
    const range = editingRange(state, pos);
    const tokens = tokenize(state.sliceDoc(range.from, range.to), range.from);
    const analysis = analyzeStatement(tokens, from);
    const candidates = await gatherCandidates(analysis, {
      dialect,
      catalog,
      upperKeywords: state.facet(upperKeywords),
    });
    if (context.aborted) return null;
    const kinds = state.facet(completionKinds);
    const wanted = candidates.filter((candidate) => offered(candidate, kinds));
    const ranked = rankCandidates(wanted, typed, analysis.scope.kind);
    return { from, options: ranked.map(toCompletion), filter: false };
  };
}
