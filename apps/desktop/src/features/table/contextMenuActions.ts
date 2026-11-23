// SPDX-License-Identifier: Apache-2.0
import type { CellRange, GridContextTarget } from '@openrows/grid';
import type { ResultMeta } from '../query/run';
import { holdsColumn } from './row';
import type { RowField } from './row';
import type { TableView } from './model';
import type { RowEditing } from './useRowEditing';
import { stagedInserts } from './EditedSource';
import { selectionActions } from './selectionActions';
import { selectedCells } from './bulk/cells';
import { cellActions } from './cellMenuActions';
import { rowText } from '../../results/copy/formats';
import type { CopyTarget } from '../../results/copy/copySelection';
import type { KeyJumps } from './links/useKeyJumps';
import { resultColumnKey } from '../../results/projectedResult';
import type { IconName } from '@openrows/ui';
import { t } from './messages/lazyData';
/** A right-click target plus the grid selection it belongs to, in data coordinates. */
export type ContextTarget = GridContextTarget & {
  ranges: readonly CellRange[];
};
export type MenuSelection = ContextTarget & { result: ResultMeta };
/** Grid-level actions: copy formats and frozen columns. */
export type GridMenu = {
  copyTarget: CopyTarget;
  pinned: readonly string[];
  togglePin: (name: string) => void;
  fit?: ((column: number) => void) | undefined;
  reset?: (() => void) | undefined;
  exportRows?: ((selection: MenuSelection) => void) | undefined;
};
export type ContextMenuProps = {
  selection: MenuSelection;
  /** Copy as and Pin column; omitted where the grid offers neither. */
  grid?: GridMenu | undefined;
  editing: RowEditing;
  changeView: (update: (view: TableView) => TableView) => void;
  /** Ask for one value to stage in every selected cell. */
  onPrompt: (count: number) => void;
  /** Foreign-key navigation; omitted where the grid has none. */
  jumps?: KeyJumps | undefined;
  /** Open the header quick filter of a column; omitted where unavailable. */
  onQuickFilter?: ((column: string) => void) | undefined;
  onClose: () => void;
};
/** A submenu that collects the actions naming it, placed at the first one. */
export type ActionGroup = { label: string; icon?: IconName };
export type Action = {
  label: string;
  disabled?: boolean;
  danger?: boolean;
  /** Starts a new group: the menu draws a separator above it. */
  separated?: boolean;
  /** Leading glyph of the menu row. */
  icon?: IconName;
  /** A fixed shortcut hint (`↩`)… */
  shortcut?: string;
  /** …or the command whose (rebindable) shortcut the row shows. */
  command?: string;
  /** Shown inside this submenu instead of at the top level. */
  group?: ActionGroup;
  run: () => void;
};
export type Context = ContextMenuProps & {
  name: string;
  writable: boolean;
  run: (action: () => void) => () => void;
};
/** Contextual row/column actions; all mutation actions stage only. */
export function contextActions(
  props: ContextMenuProps,
  fields: readonly RowField[] | null,
  copy: (text: string) => void,
): Action[] {
  const { selection, editing, onClose } = props;
  const name = selection.result.columns[selection.column]?.name ?? '';
  const context: Context = {
    ...props,
    name,
    writable: canEdit(editing, selection.result),
    run: (action) => () => {
      onClose();
      action();
    },
  };
  const cells = selectedCells(selection.ranges, 2).length;
  if (selection.kind === 'column')
    return [...columnActions(context, copy), ...selectionActions(context)];
  if (cells > 1) return selectionActions(context);
  const rows = rowActions(context, fields, copy);
  if (selection.kind === 'row') return rows;
  // Copy value, then Copy row as ▸, then the other cell commands.
  const [copyValue, ...cell] = cellActions(
    context,
    fields?.[selection.column],
    copy,
  );
  const [copyRow, ...row] = rows;
  return [
    ...(copyValue ? [copyValue] : []),
    ...(copyRow ? [copyRow] : []),
    ...cell,
    ...sortActions(context, true),
    ...row,
  ];
}
function canEdit(editing: RowEditing, result: ResultMeta): boolean {
  const info = editing.info;
  return (
    !!info &&
    !info.readOnly &&
    !!info.key.length &&
    !editing.busy &&
    !editing.history.uncertain &&
    info.key.every((name) => holdsColumn(result, name))
  );
}
/** Pin or unpin the column, when the grid supports frozen columns. */
function pinActions({ grid, selection, run }: Context): Action[] {
  if (!grid) return [];
  const name = resultColumnKey(selection.result, selection.column);
  const pinned = grid.pinned.includes(name);
  return [
    {
      label: pinned ? t('unpinColumn') : t('pinColumn'),
      icon: 'pin',
      separated: true,
      run: run(() => {
        grid.togglePin(name);
      }),
    },
  ];
}
/** Sort the view by the menu's column; `cell` adds Filter column… after. */
function sortActions(context: Context, cell: boolean): Action[] {
  const { selection, changeView, run } = context;
  const key = resultColumnKey(selection.result, selection.column);
  const sort = (descending: boolean): (() => void) =>
    run(() => {
      changeView((view) => ({
        ...view,
        offset: 0,
        sort: { column: key, descending },
      }));
    });
  const filter: Action[] = context.onQuickFilter
    ? [
        {
          label: t('filterColumn'),
          ...(cell ? {} : { icon: 'filter' as const }),
          run: run(() => {
            context.onQuickFilter?.(key);
          }),
        },
      ]
    : [];
  return [
    ...(cell ? filter : []),
    {
      label: t('sortAscending'),
      icon: 'arrowUp',
      separated: !cell,
      run: sort(false),
    },
    { label: t('sortDescending'), icon: 'arrowDown', run: sort(true) },
    ...(cell ? [] : filter),
  ];
}
function columnActions(
  context: Context,
  copy: (text: string) => void,
): Action[] {
  const { selection, name, changeView, run } = context;
  const { result } = selection;
  const key = resultColumnKey(result, selection.column);
  return [
    {
      label: t('copyColumnName'),
      icon: 'copy',
      run: () => {
        copy(name);
      },
    },
    ...sortActions(context, false),
    ...pinActions(context),
    {
      label: t('hideColumn'),
      icon: 'eyeOff',
      disabled: result.columns.length <= 1,
      run: run(() => {
        changeView((view) => ({ ...view, hidden: [...view.hidden, key] }));
      }),
    },
  ];
}
/** The `Copy row as ▸` submenu; the grid's formats join it. */
const copyRowGroup = (): ActionGroup => ({
  label: t('copyRowAs'),
  icon: 'duplicate',
});
function rowActions(
  { selection, editing, writable, run }: Context,
  fields: readonly RowField[] | null,
  copy: (text: string) => void,
): Action[] {
  const { result } = selection;
  return [
    {
      label: t('tabSeparated'),
      group: copyRowGroup(),
      disabled: !fields,
      run: () => {
        if (fields) copy(rowText(fields));
      },
    },
    {
      label: t('editRow'),
      icon: 'pencil',
      disabled: !writable,
      separated: true,
      run: run(() => {
        const insert = stagedInserts(editing.history.current)[
          selection.row - result.rows
        ];
        if (insert) editing.editStaged(insert);
        else editing.open(result, selection);
      }),
    },
    {
      label: t('addRow'),
      icon: 'plus',
      command: 'insert-row',
      disabled: !writable,
      run: run(editing.insert),
    },
    {
      label: t('duplicateRow'),
      icon: 'duplicate',
      disabled: !writable,
      run: run(() => {
        editing.quick(result, selection, 'duplicate');
      }),
    },
    {
      label: t('deleteRow'),
      icon: 'trash',
      command: 'delete-row',
      disabled: !writable,
      danger: true,
      separated: true,
      run: run(() => {
        editing.quick(result, selection, 'delete');
      }),
    },
  ];
}
