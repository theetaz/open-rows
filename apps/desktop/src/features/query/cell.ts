// SPDX-License-Identifier: Apache-2.0
/** The focused result cell, as the inspector shows it (one value, not rows). */
export type SelectedCell = {
  row: number;
  column: string;
  /** Database type name, e.g. `int4`. */
  type: string | null;
  /** Display text; `null` until its tile has arrived. */
  text: string | null;
};
