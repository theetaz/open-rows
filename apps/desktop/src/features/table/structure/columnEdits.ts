// SPDX-License-Identifier: Apache-2.0
import type { DefaultDto, SchemaChangeDto } from '@openrows/ipc';
import type { AddColumn, ColumnRow } from './effective';
import type { SchemaEditing } from './useSchemaEditing';

/** One edited Structure cell. */
export type FieldEdit =
  | { field: 'name'; text: string }
  | { field: 'dataType'; text: string }
  | { field: 'nullable'; nullable: boolean }
  | { field: 'default'; default: DefaultDto }
  | { field: 'comment'; text: string };

/** Grid columns of the Structure view, in order; `#` is column 0. */
export const columnFields = [
  'number',
  'name',
  'dataType',
  'nullable',
  'check',
  'default',
  'foreignKey',
  'comment',
] as const;

function addedPatch(add: AddColumn, edit: FieldEdit): AddColumn | null {
  switch (edit.field) {
    case 'name':
      return edit.text.trim() ? { ...add, name: edit.text.trim() } : null;
    case 'dataType':
      return edit.text.trim() ? { ...add, dataType: edit.text.trim() } : null;
    case 'nullable':
      return { ...add, nullable: edit.nullable };
    case 'default':
      return { ...add, default: edit.default };
    case 'comment':
      return { ...add, comment: edit.text || null };
  }
}

function columnChange(column: string, edit: FieldEdit): SchemaChangeDto | null {
  switch (edit.field) {
    case 'name':
      return edit.text.trim()
        ? { kind: 'renameColumn', column, name: edit.text.trim() }
        : null;
    case 'dataType':
      return edit.text.trim()
        ? { kind: 'columnType', column, dataType: edit.text.trim() }
        : null;
    case 'nullable':
      return { kind: 'columnNullable', column, nullable: edit.nullable };
    case 'default':
      return { kind: 'columnDefault', column, default: edit.default };
    case 'comment':
      return { kind: 'columnComment', column, comment: edit.text || null };
  }
}

/** Stage an edited cell: a change of a catalog column, or a new column's patch. */
export function applyField(
  schema: SchemaEditing,
  row: ColumnRow,
  edit: FieldEdit,
): void {
  if (row.column !== null) {
    const change = columnChange(row.column, edit);
    if (change) schema.stage(change);
    return;
  }
  const add = schema.history.current.find((item) => item.id === row.id);
  if (add?.change.kind !== 'addColumn') return;
  const patched = addedPatch(add.change, edit);
  if (patched) schema.replace(row.id, patched);
}

/** A staged new column with a free `column_N` name. */
export function newColumn(rows: readonly ColumnRow[]): AddColumn {
  const names = new Set(rows.map((row) => row.name));
  let index = rows.length + 1;
  while (names.has(`column_${String(index)}`)) index += 1;
  return {
    kind: 'addColumn',
    name: `column_${String(index)}`,
    dataType: 'text',
    nullable: true,
    default: { kind: 'none' },
    comment: null,
  };
}

/** The text an inline editor starts with for a field. */
export function editText(row: ColumnRow, field: string): string {
  switch (field) {
    case 'name':
      return row.name;
    case 'dataType':
      return row.dataType;
    case 'default':
      return row.default ?? '';
    case 'comment':
      return row.comment ?? '';
    default:
      return '';
  }
}

/** Whether a field of `row` can be edited inline. */
export function editable(row: ColumnRow, field: string): boolean {
  if (row.dropped) return false;
  if (field === 'default') return row.generated === 'no';
  return ['name', 'dataType', 'nullable', 'comment'].includes(field);
}
