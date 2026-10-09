// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { CascadeMenu } from '../../shell/CascadeMenu';
import type { MenuEntry } from '../../shell/CascadeMenu';
import {
  copyAsMenu,
  copyFailure,
  copySelection,
} from '../../results/copy/copySelection';
import type { ResultMeta } from '../query/run';
import { readRow, shownFields } from './row';
import type { RowField } from './row';
import { BulkValueSheet } from './BulkValueSheet';
import { contextActions } from './contextMenuActions';
import { linkLabel } from './links/foreignKeys';
import type { KeyLink } from './links/foreignKeys';
import type {
  Action,
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
        if (alive) setFields(shownFields(row));
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
      props.editing.fail(new Error('Clipboard access was blocked.'));
    });
  });
  return (
    <CascadeMenu
      x={selection.x}
      y={selection.y}
      label="Table actions"
      entries={menuEntries(props, actions)}
      onClose={props.onClose}
      footer={error && <p role="alert">{error}</p>}
    />
  );
}

/** The actions as menu rows, then `Copy as ▸` for the selection. */
function menuEntries(props: Props, actions: readonly Action[]): MenuEntry[] {
  const entries: MenuEntry[] = actions.map((action) => ({
    kind: 'item',
    label: action.label,
    run: action.run,
    disabled: action.disabled ?? false,
    danger: action.danger ?? false,
  }));
  const { grid, selection, editing } = props;
  entries.push(...keyEntries(props));
  if (!grid) return entries;
  const source = {
    result: selection.result,
    ranges: selection.ranges.map((range) => ({
      ...range,
      r1: Math.min(range.r1, selection.result.rows - 1),
    })),
  };
  return [
    ...entries,
    { kind: 'separator' },
    copyAsMenu((format) => {
      copySelection(format, source, grid.copyTarget).catch(
        (failure: unknown) => {
          editing.fail(new Error(copyFailure(failure)));
        },
      );
    }),
  ];
}

/** `Open referenced row ▸` and `Referenced by ▸` for a page row (FR-GRID-07). */
function keyEntries({ jumps, selection, onClose }: Props): MenuEntry[] {
  const { result, row, kind } = selection;
  if (!jumps || kind === 'column' || row >= result.rows) return [];
  const submenu = (label: string, links: readonly KeyLink[]): MenuEntry[] =>
    links.length
      ? [
          {
            kind: 'submenu',
            label,
            entries: links.map((link) => ({
              kind: 'item',
              label: linkLabel(link),
              run: () => {
                onClose();
                jumps.open(result, row, link);
              },
            })),
          },
        ]
      : [];
  const entries = [
    ...submenu('Open referenced row', jumps.links.outgoing),
    ...submenu('Referenced by', jumps.links.incoming),
  ];
  return entries.length ? [{ kind: 'separator' }, ...entries] : [];
}
