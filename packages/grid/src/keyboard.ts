// SPDX-License-Identifier: Apache-2.0
import type { GridProps, SelectionModel } from './model';

type Cell = { row: number; column: number };
/** Inclusive data limits; `first` skips an optional row-number column. */
interface Limits {
  rows: number;
  first: number;
  last: number;
}
/** A keyboard move: the new selection and the rows the viewport scrolls with it. */
export interface KeyMove {
  selection: SelectionModel;
  /** Page Up/Down scroll by the distance the focus moved; other keys only reveal. */
  scrollRows: number;
}

const ARROWS: Partial<Record<string, readonly [number, number]>> = {
  ArrowUp: [-1, 0],
  ArrowDown: [1, 0],
  ArrowLeft: [0, -1],
  ArrowRight: [0, 1],
};

function limits(props: GridProps): Limits {
  return {
    rows: props.source.rows,
    first: props.rowHeaderColumn === 0 ? 1 : 0,
    last: props.source.columns.length - 1,
  };
}

/**
 * Arrows, Page Up/Down, Home/End and Tab on logical indices, so 10 M rows
 * cost the same as ten. `undefined` leaves the key to the browser, which lets
 * Tab past the last cell (⇧Tab before the first) move focus out of the grid.
 */
export function keyMove(
  current: SelectionModel,
  event: KeyboardEvent,
  props: GridProps,
  pageRows: number,
): KeyMove | undefined {
  const bounds = limits(props);
  if (event.key === 'Tab') {
    const cell = plainKey(event) ? tabTarget(current, bounds, event) : null;
    return cell
      ? { selection: extend(current, cell, false), scrollRows: 0 }
      : undefined;
  }
  const target = cellTarget(current, event, bounds, pageRows);
  if (!target) return undefined;
  const cell = clamp(target, bounds);
  const page = event.key === 'PageUp' || event.key === 'PageDown';
  return {
    selection: extend(current, cell, event.shiftKey),
    scrollRows: page ? cell.row - current.row : 0,
  };
}

function plainKey(event: KeyboardEvent): boolean {
  return !event.metaKey && !event.ctrlKey && !event.altKey;
}

/** Unclamped destination; ⌘/Ctrl turns arrows into edge jumps and Home/End into first/last cell. */
function cellTarget(
  { row, column }: Cell,
  event: KeyboardEvent,
  bounds: Limits,
  pageRows: number,
): Cell | undefined {
  const jump = event.metaKey || event.ctrlKey;
  if (event.key === 'PageDown' || event.key === 'PageUp') {
    if (jump || event.altKey) return undefined;
    return {
      row: row + (event.key === 'PageDown' ? pageRows : -pageRows),
      column,
    };
  }
  if (event.key === 'Home')
    return { row: jump ? 0 : row, column: bounds.first };
  if (event.key === 'End')
    return { row: jump ? bounds.rows - 1 : row, column: bounds.last };
  const delta = ARROWS[event.key];
  if (!delta) return undefined;
  const step = jump ? Number.MAX_SAFE_INTEGER : 1;
  return { row: row + delta[0] * step, column: column + delta[1] * step };
}

/** The next (⇧: previous) cell in reading order; `null` at the grid's ends. */
function tabTarget(
  { row, column }: Cell,
  bounds: Limits,
  event: KeyboardEvent,
): Cell | null {
  if (bounds.rows === 0 || bounds.last < bounds.first) return null;
  const at = Math.max(bounds.first, Math.min(bounds.last, column));
  if (event.shiftKey) {
    if (at > bounds.first) return { row, column: at - 1 };
    return row > 0 ? { row: row - 1, column: bounds.last } : null;
  }
  if (at < bounds.last) return { row, column: at + 1 };
  return row < bounds.rows - 1 ? { row: row + 1, column: bounds.first } : null;
}

function clamp({ row, column }: Cell, bounds: Limits): Cell {
  return {
    row: Math.max(0, Math.min(bounds.rows - 1, row)),
    column: Math.max(bounds.first, Math.min(bounds.last, column)),
  };
}

/** Focus `cell`; with Shift the range grows from the fixed anchor instead. */
function extend(
  current: SelectionModel,
  cell: Cell,
  shift: boolean,
): SelectionModel {
  const anchor = shift ? (current.anchor ?? current) : cell;
  return {
    row: cell.row,
    column: cell.column,
    anchor: { row: anchor.row, column: anchor.column },
    ranges: [
      {
        r0: Math.min(anchor.row, cell.row),
        r1: Math.max(anchor.row, cell.row),
        c0: Math.min(anchor.column, cell.column),
        c1: Math.max(anchor.column, cell.column),
      },
    ],
  };
}

const TABBABLE =
  'a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),[tabindex]';

/**
 * ⌃Tab / ⌃⇧Tab: focus the next or previous tabbable element outside the grid,
 * the AppKit convention for leaving a view that uses Tab itself. Without it a
 * keyboard user would have to tab through every cell of a large result.
 */
export function leaveGrid(container: HTMLElement, backward: boolean): boolean {
  const outside = [...document.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (element) =>
      element.tabIndex >= 0 &&
      !container.contains(element) &&
      // Rendered boxes only: display:none and detached subtrees have none.
      element.getClientRects().length > 0,
  );
  const following = (element: HTMLElement): boolean =>
    (container.compareDocumentPosition(element) &
      Node.DOCUMENT_POSITION_FOLLOWING) !==
    0;
  const target = backward
    ? outside.filter((element) => !following(element)).at(-1)
    : outside.find(following);
  target?.focus();
  return target !== undefined;
}
