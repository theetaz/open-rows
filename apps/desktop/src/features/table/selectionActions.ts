// SPDX-License-Identifier: Apache-2.0
import { selectedCells, selectedRows } from './bulk/cells';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { Action, Context } from './contextMenuActions';
import { t, tn } from './messages/lazyData';

const formatted = (count: number): { count: string } => ({
  count: count.toLocaleString('en-US'),
});

/**
 * Actions for a multi-cell selection (range, column or several rows). Every
 * one stages through the shared change set as a single undo step; values
 * are validated per cell first and nothing stages when any cell refuses.
 */
export function selectionActions({
  selection,
  editing,
  writable,
  run,
  onPrompt,
}: Context): Action[] {
  const { result, ranges } = selection;
  const cells = selectedCells(ranges).length;
  const rows = selectedRows(ranges).length;
  const set = (value: Parameters<typeof editing.bulk.set>[2]): (() => void) =>
    run(() => {
      editing.bulk.set(result, ranges, value);
    });
  return [
    {
      label: tn('setValueCells', cells, formatted(cells)),
      disabled: !writable,
      run: run(() => {
        onPrompt(cells);
      }),
    },
    {
      label: tn('pasteIntoCells', cells, formatted(cells)),
      disabled: !writable,
      run: run(() => {
        navigator.clipboard.readText().then((text) => {
          editing.bulk.paste(result, ranges, text);
        }, editing.fail);
      }),
    },
    { label: t('setNull'), disabled: !writable, run: set(NULL_VALUE) },
    { label: t('setDefault'), disabled: !writable, run: set(DEFAULT_VALUE) },
    { label: t('setEmpty'), disabled: !writable, run: set(textValue('')) },
    {
      label: tn('duplicateRows', rows, formatted(rows)),
      disabled: !writable,
      run: run(() => {
        editing.bulk.duplicateRows(result, ranges);
      }),
    },
    {
      label: tn('deleteRows', rows, formatted(rows)),
      disabled: !writable,
      danger: true,
      run: run(() => {
        editing.bulk.deleteRows(result, ranges);
      }),
    },
  ];
}
