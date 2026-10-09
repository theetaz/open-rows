// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef } from 'react';
import type { RefObject } from 'react';
import { useTableCommands } from '../commands';
import type { TableCommand } from '../commands';
import { canEdit } from './EditToolbar';
import { selectionText } from './copySelection';
import type { GridSelection } from './copySelection';
import { focusFilter } from './useTableKeys';
import type { RowEditing } from './useRowEditing';
import type { TableTabState } from './useTableTab';

type Target = {
  root: RefObject<HTMLElement | null>;
  state: TableTabState;
  editing: RowEditing;
  /** An open session: rows can load and stage. */
  ready: boolean;
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

function copySelected(input: Input): void {
  const selection = currentSelection(input);
  if (!selection) return;
  selectionText(selection)
    .then((text) => navigator.clipboard.writeText(text))
    .catch(input.editing.fail);
}

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
      if (data && staged) editing.discard();
    },
    'insert-row': () => {
      if (data && canEdit(editing, input.ready)) editing.insert();
    },
    'delete-row': () => {
      if (data) deleteSelected(input);
    },
    'copy-selection': () => {
      if (data) copySelected(input);
    },
    'focus-filter': () => {
      if (!data) actions.mode('data');
      focusFilter(input.root.current, actions);
    },
    'toggle-columns': actions.toggleColumns,
    refresh: actions.reload,
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
