// SPDX-License-Identifier: Apache-2.0
import type { CellRange, GridContextTarget } from '@openrows/grid';
import type { ResultMeta } from '../query/run';
import { holdsColumn } from './row';
import type { RowField } from './row';
import { filterRow } from './model';
import { withQuickFilter } from './filters/groups';
import type { TableView } from './model';
import type { RowEditing } from './useRowEditing';
import { stagedInserts } from './EditedSource';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import { fieldActions } from './fieldActions';
import { selectionActions } from './selectionActions';
import { selectedCells } from './bulk/cells';
import { rowText } from '../../results/copy/formats';
import type { TableColumnDto } from '@openrows/ipc';
import type { CopyTarget } from '../../results/copy/copySelection';
import type { KeyJumps } from './links/useKeyJumps';
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
export type Action = {
  label: string;
  disabled?: boolean;
  danger?: boolean;
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
  return selection.kind === 'row'
    ? rows
    : [...cellActions(context, fields?.[selection.column], copy), ...rows];
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
function pinActions({ grid, name, run }: Context): Action[] {
  if (!grid) return [];
  const pinned = grid.pinned.includes(name);
  return [
    {
      label: pinned ? 'Unpin column' : 'Pin column',
      run: run(() => {
        grid.togglePin(name);
      }),
    },
  ];
}
function columnActions(
  context: Context,
  copy: (text: string) => void,
): Action[] {
  const { selection, name, changeView, run } = context;
  const { result } = selection;
  const sort = (descending: boolean): void => {
    changeView((view) => ({
      ...view,
      offset: 0,
      sort: { column: name, descending },
    }));
  };
  return [
    {
      label: 'Copy column name',
      run: () => {
        copy(name);
      },
    },
    {
      label: 'Sort ascending',
      run: run(() => {
        sort(false);
      }),
    },
    {
      label: 'Sort descending',
      run: run(() => {
        sort(true);
      }),
    },
    ...(context.onQuickFilter
      ? [
          {
            label: 'Filter column…',
            run: run(() => {
              context.onQuickFilter?.(name);
            }),
          },
        ]
      : []),
    ...pinActions(context),
    {
      label: 'Hide column',
      disabled: result.columns.length <= 1,
      run: run(() => {
        changeView((view) => ({ ...view, hidden: [...view.hidden, name] }));
      }),
    },
  ];
}
function rowActions(
  { selection, editing, writable, run }: Context,
  fields: readonly RowField[] | null,
  copy: (text: string) => void,
): Action[] {
  const { result } = selection;
  return [
    {
      label: 'Edit row…',
      disabled: !writable,
      run: run(() => {
        const insert = stagedInserts(editing.history.current)[
          selection.row - result.rows
        ];
        if (insert) editing.editStaged(insert);
        else editing.open(result, selection);
      }),
    },
    {
      label: 'Copy row',
      disabled: !fields,
      run: () => {
        if (fields) copy(rowText(fields));
      },
    },
    { label: 'Add row', disabled: !writable, run: run(editing.insert) },
    {
      label: 'Duplicate row',
      disabled: !writable,
      run: run(() => {
        editing.quick(result, selection, 'duplicate');
      }),
    },
    {
      label: 'Delete row',
      disabled: !writable,
      danger: true,
      run: run(() => {
        editing.quick(result, selection, 'delete');
      }),
    },
  ];
}
function cellActions(
  { selection, name, editing, changeView, writable, run }: Context,
  field: RowField | undefined,
  copy: (text: string) => void,
): Action[] {
  const editColumn = editing.info?.columns.find((item) => item.name === name);
  const cellWritable = writable && !editing.info?.protected.includes(name);
  const set = (value: CellValue): void => {
    editing.stageCell(
      editing.draftFor(selection.result, selection.row),
      name,
      value,
    );
  };
  return [
    {
      label: 'Edit value…',
      disabled: !cellWritable,
      run: run(() => {
        editing.edit({ row: selection.row, column: selection.column });
      }),
    },
    {
      label: 'Copy value',
      disabled: !field,
      run: () => {
        if (field) copy(field.state === 'empty' ? '' : field.text);
      },
    },
    {
      label: 'Paste value',
      disabled: !cellWritable,
      run: run(() => {
        navigator.clipboard.readText().then((text) => {
          set(textValue(text));
        }, editing.fail);
      }),
    },
    {
      label: 'Filter by value',
      disabled: !field,
      run: run(() => {
        if (field) changeView((view) => filterValue(view, field));
      }),
    },
    ...markerActions(editColumn, cellWritable).map(([label, value, ok]) => ({
      label,
      disabled: !ok,
      run: run(() => {
        set(value);
      }),
    })),
  ];
}

/** Set NULL / DEFAULT / EMPTY with the inspector menu's enablement rules. */
function markerActions(
  column: TableColumnDto | undefined,
  writable: boolean,
): readonly (readonly [string, CellValue, boolean])[] {
  const allowed = fieldActions(column, !writable, null);
  return [
    ['Set NULL', NULL_VALUE, allowed.null],
    ['Set DEFAULT', DEFAULT_VALUE, allowed.default],
    ['Set EMPTY', textValue(''), allowed.empty],
  ];
}

function filterValue(view: TableView, field: RowField): TableView {
  const row = {
    ...filterRow(view.nextRow, field.name),
    operator:
      field.state === 'null'
        ? ('isNull' as const)
        : field.state === 'empty'
          ? ('isEmpty' as const)
          : ('equal' as const),
    value: field.state === 'normal' ? field.text : '',
  };
  return withQuickFilter(view, row);
}
