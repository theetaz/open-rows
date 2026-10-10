// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ResultMeta } from '../query/run';
import { useAggregate } from './aggregate/useAggregate';
import type { PositionMemory } from './gridPosition';
import type { GridSelection } from './gridSelection';
import { useKeyJumps } from './links/useKeyJumps';
import type { KeyJumps } from './links/useKeyJumps';
import { tableTarget } from './model';
import type { ShowRow } from './row';
import { bulkRowsOf, useBulkInspector } from './useBulkInspector';
import { useDiscardReload } from './useDiscardReload';
import type { DiscardReload } from './useDiscardReload';
import type { RowEditing } from './useRowEditing';
import { useSelectedRow } from './useSelectedRow';
import type { TableActions, TableTabState } from './useTableTab';
import type { TableObject } from '../query/tabs';

type Args = {
  table: TableObject;
  profileId: string;
  sessionId: string | null;
  engine: import('@openrows/ipc').EngineDto;
  state: TableTabState;
  editing: RowEditing;
  /** A session is open: rows load and stage. */
  ready: boolean;
  onRow: ShowRow;
  onOpenTable?: ((schema: string, table: string) => void) | undefined;
};

/** The data view's selection-driven tools (FR-GRID-07/09/10/14, FR-DATA-08). */
export type DataTools = {
  selection: GridSelection | null;
  /** The grid's selection changed. */
  onSelection: (selection: GridSelection) => void;
  /** The grid's focused row changed (single-row inspector). */
  onRow: (result: ResultMeta, row: number) => void;
  jumps: KeyJumps;
  /** Footer statistics of a multi-cell selection. */
  aggregate: string | null;
  flow: DiscardReload;
  /** The tab's actions, with a confirming Reload and a Form toggle that keeps the row. */
  actions: TableActions;
  position: React.RefObject<PositionMemory>;
  /** The page row the form view shows. */
  formRow: number;
  setFormRow: (row: number) => void;
};

/**
 * Selection, inspector (single or bulk), foreign keys, aggregates, form
 * row and discard/reload of a table tab's data view.
 */
export function useDataTools(args: Args): DataTools {
  const { state, editing, ready } = args;
  const [picked, setSelection] = useState<GridSelection | null>(null);
  // A selection of a replaced page no longer applies.
  const selection =
    picked && picked.result.id === state.result?.id ? picked : null;
  const [formRow, setFormRow] = useState(0);
  /** The page whose selection spans several rows (the bulk inspector shows it). */
  const bulk = useRef<string | null>(null);
  const position = useRef<PositionMemory>({ position: null, restore: false });
  const flow = useDiscardReload({
    editing,
    reload: state.actions.reload,
    position,
  });
  const onRow = useSelectedRow({
    table: args.table.name,
    offset: state.view.offset,
    editing,
    ready,
    onRow: (row) => {
      if (row.source?.resultId !== bulk.current) args.onRow(row);
    },
  });
  useBulkInspector(
    { table: args.table.name, editing, ready, onRow: args.onRow },
    selection,
  );
  const jumps = useKeyJumps({
    target: tableTarget(args.table, args.profileId),
    sessionId: ready ? args.sessionId : null,
    engine: args.engine,
    onOpenTable: args.onOpenTable,
    onError: editing.fail,
  });
  return {
    selection,
    onSelection: (next) => {
      bulk.current = bulkRowsOf(next) ? next.result.id : null;
      setSelection(next);
    },
    onRow,
    jumps,
    aggregate: useAggregate(selection),
    flow,
    actions: {
      ...state.actions,
      reload: flow.reload,
      toggleForm: () => {
        toggleForm(state, { selection, formRow, setFormRow, position });
      },
    },
    position,
    formRow,
    setFormRow,
  };
}

/** Enter the form at the focused row, or return to the grid at the form's row. */
function toggleForm(
  state: TableTabState,
  {
    selection,
    formRow,
    setFormRow,
    position,
  }: {
    selection: GridSelection | null;
    formRow: number;
    setFormRow: (row: number) => void;
    position: React.RefObject<PositionMemory>;
  },
): void {
  if (state.view.form) {
    const previous = position.current.position;
    position.current = {
      position: {
        x: previous?.x ?? 0,
        y: previous?.y ?? 0,
        row: formRow,
        column: previous?.column ?? (state.view.rowNumbers ? 1 : 0),
      },
      restore: true,
    };
  } else setFormRow(selection?.cell.row ?? 0);
  state.actions.toggleForm();
}
