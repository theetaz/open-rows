// SPDX-License-Identifier: Apache-2.0
import type { CellValueDto } from '@openrows/ipc';

/** A staged field value: NULL, the column DEFAULT, or exact PostgreSQL text input. */
export type CellValue = CellValueDto;

export const NULL_VALUE: CellValue = { kind: 'null' };
export const DEFAULT_VALUE: CellValue = { kind: 'default' };

/** Exact text input; `'DEFAULT'` or `'NULL'` here are ordinary strings. */
export function textValue(value: string): CellValue {
  return { kind: 'text', value };
}

/** The value of an exact original snapshot field (`null` is SQL NULL). */
export function fromOriginal(original: string | null): CellValue {
  return original === null ? NULL_VALUE : textValue(original);
}

/** Staging `value` leaves the original as it is; DEFAULT always counts as a change. */
export function unchanged(
  value: CellValue,
  original: string | null | undefined,
): boolean {
  if (original === undefined || value.kind === 'default') return false;
  return value.kind === 'null'
    ? original === null
    : original !== null && value.value === original;
}

/** The editable text of a value; markers (NULL, DEFAULT) edit as an empty string. */
export function editableText(value: CellValue): string {
  return value.kind === 'text' ? value.value : '';
}

/** How a staged value reads in the grid and inspector: markers are muted. */
export function stagedDisplay(value: CellValue): {
  text: string;
  placeholder: boolean;
} {
  if (value.kind === 'null') return { text: 'NULL', placeholder: true };
  if (value.kind === 'default') return { text: 'DEFAULT', placeholder: true };
  return value.value
    ? { text: value.value, placeholder: false }
    : { text: 'EMPTY', placeholder: true };
}
