// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import type { GridContextTarget } from '@openrows/grid';
import { PointMenu } from '../../shell/PointMenu';
import type { ResultMeta } from '../query/run';
import { readRow } from './row';
import type { RowField } from './row';
import { contextActions } from './contextMenuActions';
import type {
  ContextMenuProps as Props,
  MenuSelection as Selection,
} from './contextMenuActions';

/** Own the menu above the imperative canvas; opening a menu never changes data. */
export function useTableContext(props: Omit<Props, 'selection' | 'onClose'>): {
  show: (result: ResultMeta, target: GridContextTarget) => void;
  menu: React.JSX.Element | null;
} {
  const [selection, setSelection] = useState<Selection | null>(null);
  const close = useCallback(() => {
    setSelection(null);
  }, []);
  return {
    show: (result, target) => {
      setSelection({ ...target, result });
    },
    menu: selection && (
      <TableContextMenu
        key={`${selection.result.id}:${String(selection.row)}:${String(selection.column)}`}
        {...props}
        selection={selection}
        onClose={close}
      />
    ),
  };
}

function TableContextMenu(props: Props): React.JSX.Element {
  const [fields, setFields] = useState<readonly RowField[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { selection } = props;
  useEffect(() => {
    // Staged inserts have no server row to read yet.
    if (selection.kind === 'column' || selection.row >= selection.result.rows)
      return;
    let alive = true;
    readRow(selection.result, selection.row).then(
      (row) => {
        if (alive) setFields(row);
      },
      (failure: unknown) => {
        if (alive)
          setError(
            failure instanceof Error ? failure.message : String(failure),
          );
      },
    );
    return () => {
      alive = false;
    };
  }, [selection]);
  const actions = contextActions(props, fields, (text) => {
    navigator.clipboard.writeText(text).then(props.onClose, () => {
      setError('Clipboard access was blocked.');
    });
  });
  return (
    <PointMenu
      x={selection.x}
      y={selection.y}
      label="Table actions"
      onClose={props.onClose}
    >
      {actions.map((action) => (
        <button
          type="button"
          role="menuitem"
          key={action.label}
          disabled={action.disabled}
          data-danger={action.danger}
          onClick={action.run}
        >
          {action.label}
        </button>
      ))}
      {error && <p role="alert">{error}</p>}
    </PointMenu>
  );
}
