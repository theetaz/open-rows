// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { useSelectedRow } from '../table/useSelectedRow';
import { useBulkInspector, bulkRowsOf } from '../table/useBulkInspector';
import { useAggregate } from '../table/aggregate/useAggregate';
import { useKeyJumps } from '../table/links/useKeyJumps';
import type { KeyJumps } from '../table/links/useKeyJumps';
import type { GridSelection } from '../table/gridSelection';
import type { ResultMeta } from './run';
import type { SelectedCell } from './cell';
import type { useResultEditing } from './useResultEditing';
import { storedColumn, storedRow } from '../../results/projectedResult';

type Editing = ReturnType<typeof useResultEditing>;
type ResultTools = {
  selection: GridSelection | null;
  jumps: KeyJumps;
  aggregate: string | null;
  onRow: (row: number) => void;
  onSelection: (
    cell: GridSelection['cell'],
    ranges: GridSelection['ranges'],
  ) => void;
};
/** Shared row inspector, bulk fields, foreign-key navigation and selection aggregates. */
export function useResultTools(
  args: Editing & {
    result: ResultMeta;
    original: ResultMeta;
    engine: Parameters<typeof useKeyJumps>[0]['engine'];
    onCell: (cell: SelectedCell) => void;
  },
): ResultTools {
  const [picked, setPicked] = useState<GridSelection | null>(null);
  const selection = picked?.result === args.result ? picked : null;
  const bulk = useRef(false);
  useEffect(() => {
    bulk.current = false;
  }, [args.result]);
  const details = {
    table: args.target.table || 'Query result',
    editing: args.editing,
    ready: args.ready,
    onRow: args.workspace?.onRow ?? (() => undefined),
  };
  const selectRow = useSelectedRow({
    ...details,
    onRow: (row) => {
      if (!bulk.current) details.onRow(row);
    },
  });
  useBulkInspector(details, selection);
  const jumps = useKeyJumps({
    target: args.target,
    sessionId: args.mapped ? (args.workspace?.sessionId ?? null) : null,
    engine: args.engine,
    onOpenTable: args.workspace?.onOpenTable,
    onError: args.editing.fail,
  });
  return {
    selection,
    jumps,
    aggregate: useAggregate(selection),
    onRow: (row: number) => {
      selectRow(args.result, row);
    },
    onSelection: (
      cell: GridSelection['cell'],
      ranges: GridSelection['ranges'],
    ) => {
      const next = { result: args.result, cell, ranges };
      bulk.current = bulkRowsOf(next) !== null;
      setPicked(next);
      const index = storedColumn(args.result, cell.column);
      const column = args.original.columns[index];
      // Details owns table/result rows; standalone previews retain cell inspection.
      if (!args.workspace && cell.row < args.result.rows)
        args.onCell({
          result: args.original,
          row: storedRow(args.result, cell.row),
          index,
          column: column?.name ?? '',
          type: column?.sourceType ?? null,
          text: null,
        });
    },
  };
}
