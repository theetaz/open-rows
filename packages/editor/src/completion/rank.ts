// SPDX-License-Identifier: Apache-2.0
import type { CompletionKind } from './catalog';
import type { CompletionScope } from './context';

/** One completion row before ranking. */
export type Candidate = {
  label: string;
  kind: CompletionKind;
  /** Right-aligned muted text: `keyword`, a schema or a table name. */
  detail: string;
  /** 0 current schema, 1 other user schemas, 2 system schemas and built-ins. */
  schemaRank: number;
  /** Lower comes first among equals: keyword popularity, column ordinal. */
  priority: number;
  /** Text inserted instead of the label (quoted names). */
  apply?: string;
};

/** Kinds offered per scope, most relevant first. */
const KIND_ORDER: Record<CompletionScope['kind'], readonly CompletionKind[]> = {
  start: ['keyword'],
  relation: ['table', 'view', 'schema', 'function', 'keyword'],
  member: ['column', 'table', 'view', 'function'],
  expression: ['column', 'function', 'keyword', 'table', 'view', 'schema'],
};

/** Rows rendered at most; CodeMirror shows ten at a time. */
const MAX_OPTIONS = 200;

type Ranked = {
  candidate: Candidate;
  tier: number;
  kind: number;
  exact: number;
};

/** 0 prefix, 1 substring, -1 no match; matching ignores case. */
function tierOf(label: string, needle: string): number {
  if (label.startsWith(needle)) return 0;
  return label.includes(needle) ? 1 : -1;
}

function compare(a: Ranked, b: Ranked): number {
  return (
    a.tier - b.tier ||
    a.kind - b.kind ||
    a.exact - b.exact ||
    a.candidate.schemaRank - b.candidate.schemaRank ||
    a.candidate.priority - b.candidate.priority ||
    a.candidate.label.localeCompare(b.candidate.label)
  );
}

/**
 * Filter and order candidates for the typed prefix: prefix matches before
 * substring matches, then the kinds the scope expects, exact names, the
 * current schema before others (system schemas last), common keywords, then
 * alphabetical order. Duplicate rows (same label, kind and detail) collapse.
 */
export function rankCandidates(
  candidates: readonly Candidate[],
  typed: string,
  scope: CompletionScope['kind'],
): Candidate[] {
  const needle = typed.toLowerCase();
  const order = KIND_ORDER[scope];
  const seen = new Set<string>();
  const ranked: Ranked[] = [];
  for (const candidate of candidates) {
    const label = candidate.label.toLowerCase();
    const key = `${candidate.kind}\n${label}\n${candidate.detail}`;
    const tier = tierOf(label, needle);
    const kind = order.indexOf(candidate.kind);
    if (tier < 0 || kind < 0 || seen.has(key)) continue;
    seen.add(key);
    ranked.push({ candidate, tier, kind, exact: label === needle ? 0 : 1 });
  }
  return ranked
    .sort(compare)
    .slice(0, MAX_OPTIONS)
    .map((item) => item.candidate);
}
