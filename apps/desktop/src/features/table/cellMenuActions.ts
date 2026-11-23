// SPDX-License-Identifier: Apache-2.0
import type { TableColumnDto } from '@openrows/ipc';
import type { RowField } from './row';
import { filterRow } from './model';
import type { TableView } from './model';
import { withQuickFilter } from './filters/groups';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import { fieldActions } from './fieldActions';
import { resultColumnKey } from '../../results/projectedResult';
import type { Action, ActionGroup, Context } from './contextMenuActions';
import { t } from './messages/lazyData';

/** The `Set value ▸` submenu of NULL, DEFAULT and EMPTY. */
const setValueGroup = (): ActionGroup => ({
  label: t('setValue'),
  icon: 'wand',
});

/**
 * One cell's commands, in the menu's order: Copy value, Paste value, Edit
 * value…, `Set value ▸` and Filter by value. Writes only stage.
 */
export function cellActions(
  context: Context,
  field: RowField | undefined,
  copy: (text: string) => void,
): Action[] {
  const { selection, name, editing, writable, run } = context;
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
      label: t('copyValue'),
      icon: 'copy',
      disabled: !field,
      run: () => {
        if (field) copy(field.state === 'empty' ? '' : field.text);
      },
    },
    {
      label: t('pasteValue'),
      icon: 'paste',
      disabled: !cellWritable,
      run: run(() => {
        navigator.clipboard.readText().then((text) => {
          set(textValue(text));
        }, editing.fail);
      }),
    },
    {
      label: t('editValue'),
      icon: 'pencil',
      shortcut: '↩',
      separated: true,
      disabled: !cellWritable,
      run: run(() => {
        editing.edit({ row: selection.row, column: selection.column });
      }),
    },
    ...markerActions(context, cellWritable).map(([label, value, ok]) => ({
      label,
      disabled: !ok,
      group: setValueGroup(),
      run: run(() => {
        set(value);
      }),
    })),
    filterAction(context, field),
  ];
}

/** Filter by value: `= value`, or IS NULL / is empty for markers. */
function filterAction(
  { selection, changeView, run }: Context,
  field: RowField | undefined,
): Action {
  return {
    label: t('filterValue2'),
    icon: 'filter',
    disabled: !field,
    separated: true,
    run: run(() => {
      if (field)
        changeView((view) =>
          filterValue(view, {
            ...field,
            name: resultColumnKey(selection.result, selection.column),
          }),
        );
    }),
  };
}

/** Set NULL / DEFAULT / EMPTY with the inspector menu's enablement rules. */
function markerActions(
  { editing, name }: Context,
  writable: boolean,
): readonly (readonly [string, CellValue, boolean])[] {
  const column: TableColumnDto | undefined = editing.info?.columns.find(
    (item) => item.name === name,
  );
  const allowed = fieldActions(column, !writable, null);
  return [
    [t('setNull'), NULL_VALUE, allowed.null],
    [t('setDefault'), DEFAULT_VALUE, allowed.default],
    [t('setEmpty'), textValue(''), allowed.empty],
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
