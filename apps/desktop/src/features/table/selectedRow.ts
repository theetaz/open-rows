// SPDX-License-Identifier: Apache-2.0
import { fromOriginal, stagedDisplay } from './cellValue';
import type { StagedEdit } from './editHistory';
import type { RowField } from './row';

/** The staged change whose original key values match the row, as the grid overlay matches. */
export function stagedEditFor(
  fields: readonly RowField[],
  changes: readonly StagedEdit[],
  keys: readonly string[],
): StagedEdit | undefined {
  if (!keys.length) return undefined;
  return changes.find(
    (change) =>
      change.kind !== 'insert' &&
      keys.every((name) => {
        const field = fields.find((item) => item.name === name);
        return (
          !!field &&
          field.state !== 'null' &&
          (field.state === 'empty' ? '' : field.text) === change.original[name]
        );
      }),
  );
}

/**
 * Inspector fields with staged values shown in place of the page's display
 * text, and exact original values attached once known.
 */
export function withStaged(
  fields: readonly RowField[],
  edit: StagedEdit | undefined,
  exact: Readonly<Record<string, string | null>> | null,
): RowField[] {
  const original = edit?.original ?? exact;
  const deleted = edit?.kind === 'delete';
  return fields.map((field) => {
    const staged =
      edit && !deleted && Object.hasOwn(edit.values, field.name)
        ? edit.values[field.name]
        : undefined;
    if (staged)
      return {
        ...field,
        text: stagedDisplay(staged).text,
        state: 'changed',
        value: staged,
      };
    const shown: RowField = deleted ? { ...field, state: 'deleted' } : field;
    return original && Object.hasOwn(original, field.name)
      ? { ...shown, value: fromOriginal(original[field.name] ?? null) }
      : shown;
  });
}
