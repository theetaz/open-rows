// SPDX-License-Identifier: Apache-2.0
import type { GridContextTarget } from '@openrows/grid';
import type { ResultMeta } from '../query/run';
import type { RowField } from './row';
import { filterRow, toFilters } from './model';
import type { TableView } from './model';
import type { RowEditing } from './useRowEditing';
export type MenuSelection = GridContextTarget & { result: ResultMeta };
export type ContextMenuProps = {
  selection: MenuSelection;
  editing: RowEditing;
  changeView: (update: (view: TableView) => TableView) => void;
  onClose: () => void;
};
type Action = {
  label: string;
  disabled?: boolean;
  danger?: boolean;
  run: () => void;
};
type Context = ContextMenuProps & {
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
  if (selection.kind === 'column') return columnActions(context, copy);
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
    info.key.every((name) =>
      result.columns.some((column) => column.name === name),
    )
  );
}
function columnActions(
  { selection, name, changeView, run }: Context,
  copy: (text: string) => void,
): Action[] {
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
        editing.open(result, selection);
      }),
    },
    {
      label: 'Copy row',
      disabled: !fields,
      run: () => {
        if (fields) copy(fields.map((item) => tsv(item.text)).join('\t'));
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
  const { result } = selection;
  const editColumn = editing.info?.columns.find((item) => item.name === name);
  const cellWritable = writable && !editing.info?.protected.includes(name);
  return [
    {
      label: 'Edit value…',
      disabled: !cellWritable,
      run: run(() => {
        editing.open(result, selection);
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
      label: 'Filter by value',
      disabled: !field,
      run: run(() => {
        if (field) changeView((view) => filterValue(view, field));
      }),
    },
    {
      label: 'Set NULL',
      disabled: !cellWritable || !editColumn?.nullable,
      run: run(() => {
        editing.quick(result, selection, 'null');
      }),
    },
    {
      label: 'Set empty string',
      disabled: !cellWritable || !/char|text/i.test(editColumn?.dataType ?? ''),
      run: run(() => {
        editing.quick(result, selection, 'empty');
      }),
    },
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
  return {
    ...view,
    rows: [...view.rows, row],
    applied: [...view.applied, ...toFilters([row])],
    nextRow: view.nextRow + 1,
    filtersOpen: true,
    offset: 0,
  };
}
function tsv(value: string): string {
  return /[\t\n\r"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}
