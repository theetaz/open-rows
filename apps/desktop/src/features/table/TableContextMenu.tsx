// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { PointMenu } from '../../shell/PointMenu';
import type { ResultMeta } from '../query/run';
import { readRow } from './row';
import type { RowField } from './row';
import { BulkValueSheet } from './BulkValueSheet';
import { contextActions } from './contextMenuActions';
import type {
  ContextMenuProps as Props,
  ContextTarget,
  MenuSelection as Selection,
} from './contextMenuActions';

/** Own the menu above the imperative canvas; opening a menu never changes data. */
export function useTableContext(
  props: Omit<Props, 'selection' | 'onClose' | 'onPrompt'>,
): {
  show: (result: ResultMeta, target: ContextTarget) => void;
  menu: React.JSX.Element | null;
} {
  const [selection, setSelection] = useState<Selection | null>(null);
  const [prompt, setPrompt] = useState<{
    selection: Selection;
    count: number;
  } | null>(null);
  const close = useCallback(() => {
    setSelection(null);
  }, []);
  const { editing } = props;
  if (prompt)
    return {
      show: () => undefined,
      menu: (
        <BulkValueSheet
          count={prompt.count}
          onApply={(value) => {
            const { result, ranges } = prompt.selection;
            editing.bulk.set(result, ranges, value);
          }}
          onClose={() => {
            setPrompt(null);
          }}
        />
      ),
    };
  return {
    show: (result, target) => {
      setSelection({ ...target, result });
    },
    menu: selection && (
      <TableContextMenu
        key={`${selection.result.id}:${String(selection.row)}:${String(selection.column)}`}
        {...props}
        selection={selection}
        onPrompt={(count) => {
          setPrompt({ selection, count });
        }}
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
