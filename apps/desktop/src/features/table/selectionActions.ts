// SPDX-License-Identifier: Apache-2.0
import { selectedCells, selectedRows } from './bulk/cells';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { Action, Context } from './contextMenuActions';

const plural = (count: number, noun: string): string =>
  `${count.toLocaleString('en-US')} ${noun}${count === 1 ? '' : 's'}`;

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
      label: `Set value for ${plural(cells, 'cell')}…`,
      disabled: !writable,
      run: run(() => {
        onPrompt(cells);
      }),
    },
    {
      label: `Paste into ${plural(cells, 'cell')}`,
      disabled: !writable,
      run: run(() => {
        navigator.clipboard.readText().then((text) => {
          editing.bulk.paste(result, ranges, text);
        }, editing.fail);
      }),
    },
    { label: 'Set NULL', disabled: !writable, run: set(NULL_VALUE) },
    { label: 'Set DEFAULT', disabled: !writable, run: set(DEFAULT_VALUE) },
    { label: 'Set EMPTY', disabled: !writable, run: set(textValue('')) },
    {
      label: `Duplicate ${plural(rows, 'row')}`,
      disabled: !writable,
      run: run(() => {
        editing.bulk.duplicateRows(result, ranges);
      }),
    },
    {
      label: `Delete ${plural(rows, 'row')}`,
      disabled: !writable,
      danger: true,
      run: run(() => {
        editing.bulk.deleteRows(result, ranges);
      }),
    },
  ];
}
