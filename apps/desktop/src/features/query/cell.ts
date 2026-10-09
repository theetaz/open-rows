// SPDX-License-Identifier: Apache-2.0
import type { ResultMeta } from './run';

/** The focused result cell, as the inspector shows it (one value, not rows). */
export type SelectedCell = {
  /** The result set the cell belongs to. */
  result: ResultMeta;
  row: number;
  /** Column index within the result. */
  index: number;
  column: string;
  /** Database type name, e.g. `int4`. */
  type: string | null;
  /** Display text; `null` until its tile has arrived. */
  text: string | null;
};
