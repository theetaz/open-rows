// SPDX-License-Identifier: Apache-2.0
import { isNumberText } from '../../../results/copy/sql';
import type { CopyTable } from '../../../results/copy/formats';

/** Numeric statistics over the non-NULL selected values. */
type Numeric = { sum: number; avg: number; min: number; max: number };

/**
 * Footer statistics of a selection (FR-GRID-10), as TablePlus shows them:
 * numeric columns get sum, average, minimum and maximum; any other
 * selection gets the number of distinct non-NULL values.
 */
type Aggregate = {
  /** Selected cells, NULL included. */
  count: number;
  numeric: Numeric | null;
  /** Distinct non-NULL values, for non-numeric selections. */
  distinct: number | null;
};

/** Values of the selected cells, row by row. */
function values(table: CopyTable): (string | null)[] {
  return table.rows.flatMap((row) => [...row]);
}

function numericOf(cells: readonly (string | null)[]): Numeric | null {
  let sum = 0,
    min = Infinity,
    max = -Infinity,
    count = 0;
  for (const cell of cells) {
    if (cell === null) continue;
    const value = Number(cell);
    if (!isNumberText(cell) || !Number.isFinite(value)) return null;
    sum += value;
    min = Math.min(min, value);
    max = Math.max(max, value);
    count++;
  }
  return count ? { sum, avg: sum / count, min, max } : null;
}

/** Statistics of the selected cells; numeric only when every column is numeric. */
export function aggregate(table: CopyTable): Aggregate {
  const cells = values(table);
  const numericColumns =
    table.columns.length > 0 &&
    table.columns.every((column) => column.kind === 'number');
  const numeric = numericColumns ? numericOf(cells) : null;
  return {
    count: cells.length,
    numeric,
    distinct: numeric
      ? null
      : new Set(cells.filter((cell) => cell !== null)).size,
  };
}

const format = (value: number): string =>
  value.toLocaleString('en-US', { maximumFractionDigits: 6 });

/** `Count: 4 · Sum: 10 · Avg: 2.5 · Min: 1 · Max: 4`, or count and distinct. */
export function aggregateText(result: Aggregate): string {
  const parts = [`Count: ${format(result.count)}`];
  if (result.numeric) {
    const { sum, avg, min, max } = result.numeric;
    parts.push(
      `Sum: ${format(sum)}`,
      `Avg: ${format(avg)}`,
      `Min: ${format(min)}`,
      `Max: ${format(max)}`,
    );
  } else if (result.distinct !== null)
    parts.push(`Distinct: ${format(result.distinct)}`);
  return parts.join(' · ');
}
