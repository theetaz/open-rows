// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef } from 'react';
import type { RefObject } from 'react';
import { useTableCommands } from '../commands';
import type { TableCommand } from '../commands';
import { canEdit } from './EditToolbar';
import { copySource } from './gridSelection';
import type { GridSelection } from './gridSelection';
import { emptyLayout } from '../../results/columnLayout';
import { copyFailure, copySelection } from '../../results/copy/copySelection';
import type { CopyFormat } from '../../results/copy/formats';
import { focusFilter } from './useTableKeys';
import type { RowEditing } from './useRowEditing';
import type { TableTabState } from './useTableTab';
import type { DataTools } from './useDataTools';

type Target = {
  root: RefObject<HTMLElement | null>;
  state: TableTabState;
  editing: RowEditing;
  /** An open session: rows can load and stage. */
  ready: boolean;
  /** Confirming discard/reload and foreign-key jumps. */
  tools: Pick<DataTools, 'flow' | 'jumps'>;
};

type Input = Target & {
  /** The grid's latest selection; ignored once its page is replaced. */
  selection: RefObject<GridSelection | null>;
};

/** The selection of the shown page, if any. */
function currentSelection(input: Input): GridSelection | null {
  const selection = input.selection.current;
  return selection && selection.result.id === input.state.result?.id
    ? selection
    : null;
}

/** Stage deletes for the selected rows, or the focused row. */
function deleteSelected(input: Input): void {
  const selection = currentSelection(input);
  if (!selection || !canEdit(input.editing, input.ready)) return;
  const { result, cell, ranges } = selection;
  if (ranges.length) input.editing.bulk.deleteRows(result, ranges);
  else input.editing.quick(result, cell, 'delete');
}

/** Copy the selection (or the focused row) through the shared copy engine. */
function copySelected(
  input: Input,
  format: CopyFormat,
  headers?: boolean,
): void {
  const selection = currentSelection(input);
  const grid = input.state.menu.grid;
  if (!selection || !grid) return;
  copySelection(format, copySource(selection), grid.copyTarget, headers).catch(
    (failure: unknown) => {
      input.editing.fail(new Error(copyFailure(failure)));
    },
  );
}

/** Pin or unpin the focused column. */
function pinFocused(input: Input): void {
  const selection = currentSelection(input);
  const name = selection?.result.columns[selection.cell.column]?.name;
  if (name !== undefined) input.state.menu.grid?.togglePin(name);
}

/** Copy, pin and column-layout commands of the grid. */
function gridActions(
  input: Input,
): Pick<Record<TableCommand, () => void>, GridCommand> {
  const data = input.state.view.mode === 'data';
  const copy = (format: CopyFormat, headers?: boolean) => () => {
    if (data) copySelected(input, format, headers);
  };
  return {
    'copy-selection': copy('tsv'),
    'copy-as-csv': copy('csv'),
    'copy-as-json': copy('json'),
    'copy-as-markdown': copy('markdown'),
    'copy-as-insert': copy('insert'),
    'copy-as-where-in': copy('whereIn'),
    'copy-with-headers': copy('tsv', true),
    'pin-column': () => {
      if (data) pinFocused(input);
    },
    'reset-columns': () => {
      input.state.layout.change(() => emptyLayout);
    },
  };
}

type GridCommand =
  | 'copy-selection'
  | 'copy-as-csv'
  | 'copy-as-json'
  | 'copy-as-markdown'
  | 'copy-as-insert'
  | 'copy-as-where-in'
  | 'copy-with-headers'
  | 'pin-column'
  | 'reset-columns';

/** Every table command against this tab's view, rows and staged edits. */
function tableActions(input: Input): Record<TableCommand, () => void> {
  const { editing, state } = input;
  const { actions } = state;
  const data = state.view.mode === 'data';
  const staged = editing.history.current.length > 0 && !editing.busy;
  return {
    'commit-changes': () => {
      if (data && staged && canEdit(editing, input.ready)) editing.commit();
    },
    'discard-changes': () => {
      if (data && staged) input.tools.flow.discard();
    },
    'insert-row': () => {
      if (data && canEdit(editing, input.ready)) editing.insert();
    },
    'delete-row': () => {
      if (data) deleteSelected(input);
    },
    'focus-filter': () => {
      if (!data) actions.mode('data');
      focusFilter(input.root.current, actions);
    },
    'toggle-columns': actions.toggleColumns,
    'toggle-row-form': () => {
      if (!data) actions.mode('data');
      actions.toggleForm();
    },
    'open-foreign-key': () => {
      const selection = currentSelection(input);
      if (data && selection)
        input.tools.jumps.openCell(selection.result, selection.cell);
    },
    refresh: actions.reload,
    ...gridActions(input),
  };
}

/**
 * Register this table tab as the target of the table and edit commands
 * (menu bar, palette and shortcuts) while it is the active tab. Returns
 * the grid's selection listener, which the commands act on.
 */
export function useTableCommandTarget(
  target: Target,
): (selection: GridSelection) => void {
  const selection = useRef<GridSelection | null>(null);
  useTableCommands((id) => {
    tableActions({ ...target, selection })[id]();
  });
  return useCallback((next: GridSelection) => {
    selection.current = next;
  }, []);
}
