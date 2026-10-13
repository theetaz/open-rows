// SPDX-License-Identifier: Apache-2.0
import type { DisplayCell, GridColumn } from '@openrows/grid';
import type { ColumnDto, EditInfoDto } from '@openrows/ipc';
import { temporalKind } from './temporal';

/** One choice in a boolean or enum value menu. */
export type MenuOption = { label: string; value: string | null };

/** What the inline editor needs to know about one writable column. */
export type ColumnEdit = {
  name: string;
  dataType: string;
  nullable: boolean;
  /** The column has a DEFAULT the value menu can stage. */
  hasDefault: boolean;
  /** Boolean or enum choices; `null` for free text. */
  options: readonly MenuOption[] | null;
};

const BOOLEAN = /^bool(ean)?$/i;

/** Whether a database type name is boolean (`bool`, `boolean`). */
export function isBooleanType(dataType: string): boolean {
  return BOOLEAN.test(dataType);
}
/** A writable column's editor; `null` for unknown or generated columns. */
export function columnEdit(
  info: EditInfoDto | null,
  name: string | undefined,
): ColumnEdit | null {
  const column = info?.columns.find((item) => item.name === name);
  if (!info || !column || info.protected.includes(column.name)) return null;
  const choices = BOOLEAN.test(column.dataType)
    ? [
        { label: 'TRUE', value: 'true' },
        { label: 'FALSE', value: 'false' },
      ]
    : column.enumLabels?.map((label) => ({ label, value: label }));
  return {
    name: column.name,
    dataType: column.dataType,
    nullable: column.nullable,
    hasDefault: column.default !== null,
    options: choices
      ? [
          ...choices,
          ...(column.nullable ? [{ label: 'NULL', value: null }] : []),
        ]
      : null,
  };
}

/** Canvas column kinds: booleans from the result type, enums once edit info arrives. */
export function gridKinds(
  columns: readonly ColumnDto[],
  info: EditInfoDto | null,
): GridColumn['kind'][] {
  return columns.map((column) => {
    const edit = info?.columns.find((item) => item.name === column.name);
    if (column.sourceType === 'bool' || (edit && BOOLEAN.test(edit.dataType)))
      return 'boolean';
    if (edit?.enumLabels?.length) return 'enum';
    return temporalKind(edit?.dataType ?? column.sourceType ?? '')
      ? 'temporal'
      : undefined;
  });
}

/** Editable text of a painted cell; markers (NULL, EMPTY, DEFAULT) start empty. */
export function cellText(cell: DisplayCell | undefined): string {
  if (!cell || cell.marker || cell.state === 'null' || cell.state === 'empty')
    return '';
  return cell.text;
}

/** Only multi-line values need the popover; long single-line values edit in the cell like TablePlus. */
export function isLong(value: string): boolean {
  return value.includes('\n');
}

/** JSON reformatting for the popover; unchanged when the text is not valid JSON. */
export function reformat(value: string, pretty: boolean): string {
  try {
    return JSON.stringify(JSON.parse(value), null, pretty ? 2 : undefined);
  } catch {
    return value;
  }
}
