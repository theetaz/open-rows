// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { SelectedCell } from '../query';
import type { SelectedRow, ShowRow } from '../table';

/** What the inspector's Details shows: a query cell or a table view row. */
export type Details = {
  cell: SelectedCell | null;
  row: SelectedRow | null;
  onCell: (cell: SelectedCell) => void;
  onRow: ShowRow;
};

/** The latest selection of the tab `scope` wins; other tabs' selections hide. */
export function useDetails(scope: string | undefined): Details {
  const [cell, setCell] = useState<{
    scope: string | undefined;
    value: SelectedCell;
  } | null>(null);
  const [row, setRow] = useState<{
    scope: string | undefined;
    value: SelectedRow;
  } | null>(null);
  return {
    cell: cell?.scope === scope ? (cell?.value ?? null) : null,
    row: row?.scope === scope ? (row?.value ?? null) : null,
    onCell: (selected) => {
      setCell({ scope, value: selected });
      setRow(null);
    },
    onRow: (selected) => {
      setRow({ scope, value: selected });
      setCell(null);
    },
  };
}
