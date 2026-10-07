// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { SelectedCell } from '../features/query';
import type { SelectedRow } from '../features/table';

/** What the inspector's Details shows: a query cell or a table view row. */
export type Details = {
  cell: SelectedCell | null;
  row: SelectedRow | null;
  onCell: (cell: SelectedCell) => void;
  onRow: (row: SelectedRow) => void;
};

/** The latest selection wins and brings the Details panel forward. */
export function useDetails(setPanel: (panel: string) => void): Details {
  const [cell, setCell] = useState<SelectedCell | null>(null);
  const [row, setRow] = useState<SelectedRow | null>(null);
  return {
    cell,
    row,
    onCell: (selected) => {
      setCell(selected);
      setRow(null);
      setPanel('Details');
    },
    onRow: (selected) => {
      setRow(selected);
      setCell(null);
      setPanel('Details');
    },
  };
}
