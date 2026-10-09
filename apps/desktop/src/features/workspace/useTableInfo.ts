// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import type { SelectedTable } from '../table';

/** Table info the inspector shows instead of row details (Structure/Index). */
export type TableInfoState = {
  table: SelectedTable | null;
  /** Stable, so table tabs can publish from an effect. */
  onTable: (table: SelectedTable | null) => void;
};

/** The active tab's published table info; other tabs' info is ignored. */
export function useTableInfo(scope: string | undefined): TableInfoState {
  const [table, setTable] = useState<{
    scope: string | undefined;
    value: SelectedTable;
  } | null>(null);
  const onTable = useCallback(
    (value: SelectedTable | null) => {
      setTable(value ? { scope, value } : null);
    },
    [scope],
  );
  return {
    table: table?.scope === scope ? (table?.value ?? null) : null,
    onTable,
  };
}
