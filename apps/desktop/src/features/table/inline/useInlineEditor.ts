// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type {
  CellRange,
  CellRect,
  GridDataSource,
  GridHandle,
  GridTheme,
  SelectionModel,
} from '@openrows/grid';
import type { ResultMeta } from '../../query/run';
import type { StagedEdit } from '../editHistory';
import { writableResult } from '../cellEdits';
import { editableText, fromOriginal } from '../cellValue';
import type { RowEditing } from '../useRowEditing';
import { cellText, columnEdit, isLong } from './cellKind';
import type { ColumnEdit } from './cellKind';
import { dataRanges, inRanges, selectedCells } from '../bulk/cells';

/** One-line input over the cell, multi-line popover, or a value menu. */
export type EditorMode = 'input' | 'popover' | 'menu';
export type Move = 'next' | 'previous' | 'up' | 'down';
/** The open editor; `row`/`column` are result (data) coordinates. */
export type OpenEditor = {
  row: number;
  column: number;
  edit: ColumnEdit;
  mode: EditorMode;
  value: string;
  /** The exact value has loaded (or the user typed first). */
  ready: boolean;
  dirty: boolean;
  /** Host-relative cell geometry, and its bottom-left in the window. */
  rect: CellRect;
  anchor: { x: number; y: number };
  font: string;
  draft: Promise<StagedEdit>;
};
export type InlineEditor = {
  open: OpenEditor | null;
  start: (cell: { row: number; column: number }, seed?: string) => void;
  change: (value: string) => void;
  commit: (value: string | null, move?: Move) => void;
  cancel: () => void;
  reposition: () => void;
};
type Args = {
  host: React.RefObject<HTMLElement | null>;
  handle: React.RefObject<GridHandle | undefined>;
  theme: React.RefObject<GridTheme>;
  /** Page and staged-insert cells in result (data) coordinates. */
  source: React.RefObject<GridDataSource | undefined>;
  /** The grid's current selection, in grid coordinates. */
  selection: React.RefObject<SelectionModel>;
  editing: RowEditing;
  result: ResultMeta;
  rowNumbers: boolean;
};

/** Edit one cell in place; Escape never stages, commits go through the staging queue. */
export function useInlineEditor(args: Args): InlineEditor {
  const [open, setOpen] = useState<OpenEditor | null>(null);
  const active = useRef<Promise<StagedEdit> | null>(null);
  const shift = args.rowNumbers ? 1 : 0;
  const close = (message: string, move?: Move): void => {
    active.current = null;
    setOpen(null);
    const grid = args.handle.current;
    if (open && move) moveFrom(args, open, move);
    grid?.announce?.(message);
    grid?.focus?.();
  };
  return {
    open,
    start: (cell, seed) => {
      const next = opening(args, cell, seed);
      if (!next) return;
      active.current = next.draft;
      setOpen(next);
      args.handle.current?.announce?.(`Editing ${next.edit.name}`);
      next.draft.then(
        (row) => {
          setOpen((state) =>
            state?.draft === next.draft ? loaded(state, row, args) : state,
          );
        },
        () => {
          if (active.current !== next.draft) return;
          active.current = null;
          setOpen(null);
        },
      );
    },
    change: (value) => {
      setOpen((state) => state && { ...state, value, dirty: true });
    },
    commit: (value, move) => {
      if (!open || active.current !== open.draft) return;
      const changed = open.mode === 'menu' || open.dirty || value === null;
      const many = changed ? selectionAround(args, open) : null;
      if (many) args.editing.bulk.set(args.result, many, fromOriginal(value));
      else if (changed)
        args.editing.stageCell(open.draft, open.edit.name, fromOriginal(value));
      close(
        many
          ? `${open.edit.name} staged for the selection`
          : changed
            ? `${open.edit.name} staged`
            : 'Unchanged',
        move,
      );
    },
    cancel: () => {
      if (open && active.current === open.draft) close('Edit cancelled');
    },
    reposition: () => {
      if (!open) return;
      const rect = args.handle.current?.cellRect?.(
        open.row,
        open.column + shift,
      );
      if (!rect) return;
      const anchor = anchorOf(args.host.current, rect);
      setOpen((state) =>
        state?.draft === open.draft ? { ...state, rect, anchor } : state,
      );
    },
  };
}

/** The multi-cell selection the edited cell belongs to, if any: the value fills all of it. */
function selectionAround(
  args: Args,
  cell: { row: number; column: number },
): readonly CellRange[] | null {
  const ranges = dataRanges(args.selection.current.ranges, args.rowNumbers);
  return selectedCells(ranges, 2).length > 1 && inRanges(ranges, cell)
    ? ranges
    : null;
}

/** Validate the target and build the editor; announces why a cell cannot be edited. */
function opening(
  args: Args,
  cell: { row: number; column: number },
  seed: string | undefined,
): OpenEditor | null {
  const target = editable(args, cell);
  if (!target) return null;
  const { edit, rect } = target;
  const value =
    seed ?? cellText(args.source.current?.cell(cell.row, cell.column));
  const { fontSize, fontFamily } = args.theme.current;
  const long = seed === undefined && isLong(value, edit, room(rect, fontSize));
  return {
    ...cell,
    edit,
    mode: edit.options ? 'menu' : long ? 'popover' : 'input',
    value,
    ready: seed !== undefined,
    dirty: seed !== undefined,
    rect,
    anchor: anchorOf(args.host.current, rect),
    font: `${String(fontSize)}px ${fontFamily}`,
    draft: args.editing.draftFor(args.result, cell.row),
  };
}

/** The writable column and its on-screen rectangle, or why there is none. */
function editable(
  { editing, result, handle, rowNumbers }: Args,
  cell: { row: number; column: number },
): { edit: ColumnEdit; rect: CellRect } | null {
  const grid = handle.current;
  if (!grid) return null;
  if (!writableResult(editing.info, editing.history, result)) {
    grid.announce?.('Rows cannot be edited here');
    return null;
  }
  const name = result.columns[cell.column]?.name;
  const edit = columnEdit(editing.info, name);
  const rect = grid.cellRect?.(cell.row, cell.column + (rowNumbers ? 1 : 0));
  if (edit && rect) return { edit, rect };
  grid.announce?.(`${name ?? 'This column'} cannot be edited`);
  return null;
}

/** Replace the painted text with the exact staged or original value, unless the user typed. */
function loaded(state: OpenEditor, row: StagedEdit, args: Args): OpenEditor {
  if (state.dirty) return { ...state, ready: true };
  const name = state.edit.name;
  const staged = row.values[name];
  const exact = staged ? editableText(staged) : row.original[name];
  const value = exact ?? '';
  const long =
    state.mode === 'input' &&
    isLong(value, state.edit, room(state.rect, args.theme.current.fontSize));
  return { ...state, value, ready: true, mode: long ? 'popover' : state.mode };
}

function room(
  rect: CellRect,
  fontSize: number,
): { width: number; charWidth: number } {
  return { width: rect.width - 16, charWidth: fontSize * 0.6 };
}

function anchorOf(
  host: HTMLElement | null,
  rect: CellRect,
): { x: number; y: number } {
  const bounds = host?.getBoundingClientRect();
  return {
    x: (bounds?.left ?? 0) + rect.x,
    y: (bounds?.top ?? 0) + rect.y + rect.height,
  };
}

/** Tab, Shift+Tab, ↑ and ↓ commit, then move the grid focus. */
function moveFrom(args: Args, open: OpenEditor, move: Move): void {
  const rows = args.source.current?.rows ?? 0,
    columns = args.result.columns.length;
  const row =
    move === 'up'
      ? Math.max(0, open.row - 1)
      : move === 'down'
        ? Math.min(rows - 1, open.row + 1)
        : open.row;
  const column =
    move === 'next'
      ? Math.min(columns - 1, open.column + 1)
      : move === 'previous'
        ? Math.max(0, open.column - 1)
        : open.column;
  args.handle.current?.select?.(row, column + (args.rowNumbers ? 1 : 0));
}
