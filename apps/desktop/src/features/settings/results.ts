// SPDX-License-Identifier: Apache-2.0

/** Bounds the core accepts for the result memory budget, in MiB. */
const MIN_BUDGET_MB = 64;
const MAX_BUDGET_MB = 65_536;

/**
 * Result store preferences; `settings.json` keeps them under `results`.
 * The core reads them at launch (docs/06 §3); there is no pane yet.
 */
type ResultSettings = {
  /** Arrow memory kept in RAM across results before spilling to disk. */
  memoryBudgetMb: number;
};

const defaultResults: ResultSettings = { memoryBudgetMb: 512 };

/** Key in web storage (and `results` in settings.json). */
export const RESULTS_KEY = 'openrows.results';

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseResults(stored: unknown): ResultSettings {
  if (typeof stored !== 'object' || stored === null) return defaultResults;
  const budget = (stored as Record<string, unknown>).memoryBudgetMb;
  const valid =
    typeof budget === 'number' &&
    Number.isInteger(budget) &&
    budget >= MIN_BUDGET_MB &&
    budget <= MAX_BUDGET_MB;
  return { memoryBudgetMb: valid ? budget : defaultResults.memoryBudgetMb };
}
