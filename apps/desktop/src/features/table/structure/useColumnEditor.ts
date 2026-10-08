// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent, RefObject } from 'react';
import { typedDefault } from './effective';
import type { ColumnRow } from './effective';
import {
  applyField,
  columnFields,
  commonTypes,
  defaultPresets,
  editable,
  editText,
} from './columnEdits';
import type { FieldEdit } from './columnEdits';
import type { InlineEdit } from './columnCells';
import type { Cursor } from './StructureGrid';
import type { MenuItem } from './ItemMenu';
import type { Structure } from './useStructure';

type Shown = { row: ColumnRow; index: number };
type Picker = 'dataType' | 'nullable' | 'default';
type Point = { x: number; y: number };
type Finish = 'commit' | 'cancel' | 'next' | 'blur';

/** Inline edits, pickers and keys of the Structure column grid. */
type ColumnEditorState = {
  /** Rows matching the search, with their position in the table. */
  shown: readonly Shown[];
  at: (row: number) => ColumnRow | undefined;
  startEdit: (cursor: Cursor, literal?: boolean) => void;
  finish: (how: Finish) => void;
  openPicker: (name: Picker, row: number, at: Point) => void;
  keyDown: (event: KeyboardEvent<HTMLElement>, cursor: Cursor) => boolean;
};

function fieldEdit(field: string, edit: InlineEdit): FieldEdit | null {
  switch (field) {
    case 'name':
    case 'dataType':
    case 'comment':
      return { field, text: edit.text };
    case 'default':
      return { field, default: typedDefault(edit.text, edit.literal) };
    default:
      return null;
  }
}

export function useColumnEditor(
  structure: Structure,
  grid: RefObject<HTMLDivElement | null>,
): ColumnEditorState {
  const { schema, columnGrid: state } = structure;
  const query = structure.search.trim().toLowerCase();
  const shown = structure.rows
    .map((row, index) => ({ row, index }))
    .filter(({ row }) => !query || row.name.toLowerCase().includes(query));
  const at = (row: number): ColumnRow | undefined => shown[row]?.row;
  const field = (col: number): string => columnFields[col] ?? '';
  const point = (cursor: Cursor): Point => {
    const rows = grid.current?.querySelectorAll('tbody tr');
    const box =
      rows?.[cursor.row]?.children[cursor.col]?.getBoundingClientRect();
    return { x: box?.left ?? 0, y: box?.bottom ?? 0 };
  };
  const openPicker = (name: Picker, index: number, where: Point): void => {
    const row = at(index);
    if (!row) return;
    const items = pickerItems(name, row, {
      apply: (edit) => {
        applyField(schema, row, edit);
      },
      edit: (col, literal) => () => {
        // After the menu has closed and returned focus to the grid.
        window.setTimeout(() => {
          startEdit({ row: index, col }, literal);
        }, 0);
      },
    });
    structure.setMenu({ ...where, label: `${row.name} ${name}`, items });
  };
  const startEdit = (cursor: Cursor, literal = false): void => {
    const row = at(cursor.row);
    const name = field(cursor.col);
    if (!row || !schema.enabled || !editable(row, name)) return;
    if (name === 'nullable') {
      openPicker('nullable', cursor.row, point(cursor));
      return;
    }
    state.setCursor(cursor);
    state.setEdit({
      ...cursor,
      text: literal ? '' : editText(row, name),
      literal,
    });
  };
  const finish = (how: Finish): void => {
    const edit = state.edit;
    const row = edit ? at(edit.row) : undefined;
    if (!edit || !row) return;
    const change = how === 'cancel' ? null : fieldEdit(field(edit.col), edit);
    if (change) applyField(schema, row, change);
    state.setEdit(null);
    if (how === 'next')
      state.setCursor({
        row: edit.row,
        col: Math.min(edit.col + 1, columnFields.length - 1),
      });
    if (how !== 'blur') grid.current?.focus();
  };
  return {
    shown,
    at,
    startEdit,
    finish,
    openPicker,
    keyDown: (event, cursor) =>
      gridKey(event, at(cursor.row), structure, () => {
        const name = field(cursor.col);
        if (name === 'dataType' || name === 'nullable' || name === 'default')
          openPicker(name, cursor.row, point(cursor));
      }),
  };
}

/** Alt+↓ opens the cell's picker; Delete stages the column's deletion. */
function gridKey(
  event: KeyboardEvent<HTMLElement>,
  row: ColumnRow | undefined,
  structure: Structure,
  picker: () => void,
): boolean {
  if (!row || !structure.schema.enabled) return false;
  if (event.altKey && event.key === 'ArrowDown') {
    event.preventDefault();
    picker();
    return true;
  }
  if ((event.key === 'Delete' || event.key === 'Backspace') && !row.dropped) {
    event.preventDefault();
    structure.dropRow(row);
    return true;
  }
  return false;
}

function pickerItems(
  name: Picker,
  row: ColumnRow,
  act: {
    apply: (edit: FieldEdit) => void;
    edit: (col: number, literal: boolean) => () => void;
  },
): MenuItem[] {
  if (name === 'nullable')
    return [true, false].map((nullable) => ({
      label: nullable ? 'YES' : 'NO',
      checked: row.nullable === nullable,
      run: () => {
        act.apply({ field: 'nullable', nullable });
      },
    }));
  if (name === 'dataType')
    return [
      ...commonTypes.map((type) => ({
        label: type,
        checked: row.dataType === type,
        run: () => {
          act.apply({ field: 'dataType', text: type });
        },
      })),
      { label: 'Other type…', run: act.edit(2, false) },
    ];
  return [
    {
      label: 'No default',
      checked: row.default === null,
      run: () => {
        act.apply({ field: 'default', default: { kind: 'none' } });
      },
    },
    { label: 'SQL expression…', run: act.edit(5, false) },
    { label: 'Text literal…', run: act.edit(5, true) },
    ...defaultPresets.map((sql) => ({
      label: sql,
      checked: row.default === sql,
      run: () => {
        act.apply({ field: 'default', default: { kind: 'expression', sql } });
      },
    })),
  ];
}
