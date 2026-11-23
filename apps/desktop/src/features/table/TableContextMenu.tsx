// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { Icon, Menu } from '@openrows/ui';
import type { MenuEntry } from '@openrows/ui';
import { useBindings } from '../commands';
import {
  copyAsMenu,
  copyFailure,
  copySelection,
} from '../../results/copy/copySelection';
import type { ResultMeta } from '../query/run';
import { readRow, shownFields } from './row';
import type { RowField } from './row';
import { BulkValueSheet } from './BulkValueSheet';
import { QuickFilter } from './filters/QuickFilter';
import type { QuickOperator } from './filters/QuickFilter';
import { quickFiltered } from './filters/quickFilters';
import type { FilterOperatorDto } from '@openrows/ipc';
import { contextActions } from './contextMenuActions';
import { actionEntries, separate, withGlyph } from './contextMenuEntries';
import { linkLabel } from './links/foreignKeys';
import type { KeyLink } from './links/foreignKeys';
import type {
  ContextMenuProps as Props,
  ContextTarget,
  MenuSelection as Selection,
} from './contextMenuActions';
import { t } from './messages/lazyData';

/** A header quick filter being edited: its column and where the header was clicked. */
type Quick = { column: string; x: number; y: number };

type ContextArgs = Omit<
  Props,
  'selection' | 'onClose' | 'onPrompt' | 'onQuickFilter'
> & {
  /** Header quick-filter operators of a column; omitted to offer none. */
  quickOperators?:
    | ((column: string) => readonly QuickOperator<FilterOperatorDto>[])
    | undefined;
};

/** The header quick filter popover, applying through the view's filters. */
function QuickFilterFor({
  quick,
  args,
  onClose,
}: {
  quick: Quick;
  args: ContextArgs;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <QuickFilter
      name={quick.column}
      x={quick.x}
      y={quick.y}
      operators={args.quickOperators?.(quick.column) ?? []}
      onApply={(operator, value) => {
        args.changeView((view) =>
          quickFiltered(view, quick.column, { operator, value }),
        );
      }}
      onClose={onClose}
    />
  );
}

/** Own the menu above the imperative canvas; opening a menu never changes data. */
export function useTableContext(props: ContextArgs): {
  show: (result: ResultMeta, target: ContextTarget) => void;
  menu: React.JSX.Element | null;
} {
  const [selection, setSelection] = useState<Selection | null>(null);
  const [quick, setQuick] = useState<Quick | null>(null);
  const closeQuick = useCallback(() => {
    setQuick(null);
  }, []);
  const [prompt, setPrompt] = useState<{
    selection: Selection;
    count: number;
  } | null>(null);
  const close = useCallback(() => {
    setSelection(null);
  }, []);
  const { editing } = props;
  if (quick)
    return {
      show: () => undefined,
      menu: <QuickFilterFor quick={quick} args={props} onClose={closeQuick} />,
    };
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
        onQuickFilter={
          props.quickOperators
            ? (column) => {
                setQuick({ column, x: selection.x, y: selection.y });
              }
            : undefined
        }
        onClose={close}
      />
    ),
  };
}

function TableContextMenu(props: Props): React.JSX.Element {
  const [fields, setFields] = useState<readonly RowField[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const bindings = useBindings();
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
      props.editing.fail(new Error(t('clipboardAccessWas')));
    });
  });
  const built = actionEntries(actions, bindings);
  return (
    <Menu
      at={{ x: selection.x, y: selection.y }}
      label={t('tableActions')}
      entries={menuEntries(props, built)}
      onClose={props.onClose}
      footer={error && <p role="alert">{error}</p>}
    />
  );
}

type Built = ReturnType<typeof actionEntries>;

/** Copy the selection (or the one row) in a format, reporting failures. */
function copyIn(props: Props, ranges: Props['selection']['ranges']) {
  const { grid, selection, editing } = props;
  const source = {
    result: selection.result,
    ranges: ranges.map((range) => ({
      ...range,
      r1: Math.min(range.r1, selection.result.rows - 1),
    })),
  };
  return copyAsMenu((format) => {
    if (!grid) return;
    copySelection(format, source, grid.copyTarget).catch((failure: unknown) => {
      editing.fail(new Error(copyFailure(failure)));
    });
  });
}

/**
 * The actions' rows, the foreign-key submenus before Edit row…, then the
 * grid's copy formats: inside `Copy row as ▸` for one cell or row, else as
 * `Copy as ▸` for the selection; layout and export last.
 */
function menuEntries(props: Props, built: Built): MenuEntry[] {
  const { grid, selection } = props;
  const entries = [...built.entries];
  const keys = keyEntries(props);
  const before = entries.findIndex(
    (entry) => entry.kind === 'item' && entry.label === t('editRow'),
  );
  if (before > 0) entries.splice(before - 1, 0, ...keys);
  else entries.push(...keys);
  if (!grid) return entries;
  const rowCopy = built.groups.get(t('copyRowAs'));
  const { result, row } = selection;
  if (rowCopy && row < result.rows) {
    const last = result.columns.length - 1;
    const formats = copyIn(props, [{ r0: row, r1: row, c0: 0, c1: last }]);
    if (formats.kind === 'submenu')
      rowCopy.push({ kind: 'separator' }, ...formats.entries);
  } else if (!rowCopy) {
    separate(entries);
    entries.push(withGlyph(copyIn(props, selection.ranges), 'copy'));
  }
  return [...entries, ...layoutEntries(props)];
}

/** `Open referenced row ▸` and `Referenced by ▸` for a page row (FR-GRID-07). */
function keyEntries({ jumps, selection, onClose }: Props): MenuEntry[] {
  const { result, row, kind } = selection;
  if (!jumps || kind === 'column' || row >= result.rows) return [];
  const submenu = (
    label: string,
    links: readonly KeyLink[],
    icon: React.JSX.Element | null,
  ): MenuEntry[] =>
    links.length
      ? [
          {
            kind: 'submenu',
            label,
            ...(icon ? { icon } : {}),
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
    ...submenu(
      t('openReferencedRow'),
      jumps.links.outgoing,
      <Icon name="arrowUpRight" />,
    ),
    ...submenu(t('referenced'), jumps.links.incoming, null),
  ];
  return entries.length ? [{ kind: 'separator' }, ...entries] : [];
}

/** Additional presentation/export actions supplied by a query-result host. */
function layoutEntries({ grid, selection, onClose }: Props): MenuEntry[] {
  if (!grid) return [];
  const actions: {
    label: string;
    icon: 'fit' | 'columns' | 'upload';
    run: () => void;
  }[] = [];
  if (selection.kind === 'column') {
    if (grid.fit)
      actions.push({
        label: t('sizeColumnFit'),
        icon: 'fit',
        run: () => grid.fit?.(selection.column),
      });
    if (grid.reset)
      actions.push({
        label: t('resetColumns'),
        icon: 'columns',
        run: grid.reset,
      });
  } else if (grid.exportRows)
    actions.push({
      label: t('exportSelectedRows'),
      icon: 'upload',
      run: () => grid.exportRows?.(selection),
    });
  const rows: MenuEntry[] = actions.map(({ label, icon, run }) => ({
    kind: 'item',
    label,
    icon: <Icon name={icon} />,
    run: () => {
      onClose();
      run();
    },
  }));
  return rows.length ? [{ kind: 'separator' }, ...rows] : [];
}
