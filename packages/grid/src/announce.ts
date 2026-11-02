// SPDX-License-Identifier: Apache-2.0
import type { CellRange, GridProps, SelectionModel } from './model';
import { cellText, rowLabel } from './shadowRows';

/** At most one selection announcement per interval while keys repeat or a drag runs. */
export const ANNOUNCE_INTERVAL_MS = 200;

/** Polite live-region messages: throttled selection changes, immediate state changes. */
interface Announcer {
  /** Announce the selection if it differs from the last one seen. */
  selection: (selection: SelectionModel) => void;
  /** A tile arrived: re-announce a focused value that was still loading. */
  refresh: (selection: SelectionModel) => void;
  /** Announce at once, e.g. entering edit mode or a staging outcome. */
  message: (text: string) => void;
  destroy: () => void;
}

export function announcer(
  status: HTMLElement,
  props: GridProps,
  initial: SelectionModel,
): Announcer {
  let last = selectionKey(initial),
    loading = false,
    timer: ReturnType<typeof setTimeout> | undefined;
  const say = (text: string): void => {
    // Repeating identical text would not be read again; vary it invisibly.
    status.textContent = status.textContent === text ? `${text}\u00a0` : text;
  };
  const schedule = (): void => {
    timer ??= setTimeout(() => {
      timer = undefined;
      loading = awaitingValue(props, props.selection);
      say(describeSelection(props, props.selection));
    }, ANNOUNCE_INTERVAL_MS);
  };
  return {
    selection: (selection) => {
      const key = selectionKey(selection);
      if (key === last) return;
      last = key;
      schedule();
    },
    refresh: (selection) => {
      if (loading && selectionKey(selection) === last) schedule();
    },
    message: (text) => {
      clearTimeout(timer);
      timer = undefined;
      loading = false;
      say(text);
    },
    destroy: () => {
      clearTimeout(timer);
    },
  };
}

/** "Row 12, name: Alice" for one cell; "Selected 3 rows by 2 columns" for a range. */
export function describeSelection(
  props: GridProps,
  selection: SelectionModel,
): string {
  const { ranges } = selection;
  const only = ranges.length === 1 ? ranges[0] : undefined;
  if (ranges.length === 0 || (only && singleCell(only, selection)))
    return focusText(props, selection);
  if (!only) return `Selected ${count(ranges.length, 'range')}`;
  const rows = only.r1 - only.r0 + 1,
    columns = only.c1 - only.c0 + 1;
  if (rows === props.source.rows && columns === 1)
    return `Selected column ${title(props, only.c0)}`;
  if (rows === 1 && columns >= dataColumns(props))
    return `Selected row ${rowLabel(props, only.r0)}`;
  return `Selected ${count(rows, 'row')} by ${count(columns, 'column')}`;
}

function focusText(props: GridProps, { row, column }: SelectionModel): string {
  if (props.source.rows === 0) return 'No rows';
  if (column === props.rowHeaderColumn) return `Row ${rowLabel(props, row)}`;
  return `Row ${rowLabel(props, row)}, ${title(props, column)}: ${cellText(props, row, column)}`;
}

/** A single focused data cell whose tile has not arrived yet. */
function awaitingValue(props: GridProps, selection: SelectionModel): boolean {
  const { row, column, ranges } = selection;
  const only = ranges.length === 1 ? ranges[0] : undefined;
  return (
    (ranges.length === 0 ||
      (only !== undefined && singleCell(only, selection))) &&
    row < props.source.rows &&
    column !== props.rowHeaderColumn &&
    props.source.cell(row, column) === undefined
  );
}

function singleCell(
  range: CellRange,
  { row, column }: SelectionModel,
): boolean {
  return (
    range.r0 === row &&
    range.r1 === row &&
    range.c0 === column &&
    range.c1 === column
  );
}

/** Columns a whole-row selection covers: all but the row-number gutter. */
function dataColumns(props: GridProps): number {
  const columns = props.source.columns.length;
  return props.rowHeaderColumn === undefined ? columns : columns - 1;
}

function title(props: GridProps, column: number): string {
  return props.source.columns[column]?.title ?? `column ${String(column + 1)}`;
}

const NUMBER = new Intl.NumberFormat('en-US');

function count(value: number, noun: string): string {
  return `${NUMBER.format(value)} ${noun}${value === 1 ? '' : 's'}`;
}

function selectionKey({ row, column, ranges }: SelectionModel): string {
  const parts = ranges.map(
    (range) =>
      `${String(range.r0)}:${String(range.r1)}:${String(range.c0)}:${String(range.c1)}`,
  );
  return `${String(row)},${String(column)}|${parts.join(';')}`;
}
