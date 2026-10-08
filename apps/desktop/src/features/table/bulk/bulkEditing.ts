// SPDX-License-Identifier: Apache-2.0
import type { CellRange } from '@openrows/grid';
import type { EditInfoDto, TableColumnDto } from '@openrows/ipc';
import type { ResultMeta } from '../../query/run';
import { writableResult, fieldEdit } from '../cellEdits';
import { NULL_VALUE, textValue } from '../cellValue';
import type { CellValue } from '../cellValue';
import type { EditHistory, StagedEdit } from '../editHistory';
import { stagedInserts } from '../EditedSource';
import type { TableTargetRef } from '../model';
import { duplicateRow } from '../quickEdits';
import type { EditIo } from '../useEditIo';
import {
  origin,
  parseTsv,
  pastePlan,
  selectedCells,
  selectedRows,
} from './cells';
import type { Cell } from './cells';
import { rowDrafts } from './drafts';
import { cellError, summarize } from './validate';

/** Multi-cell and multi-row staging; every action is one undo step and stages nothing on any error. */
export type BulkEditing = {
  /** Stage `value` in every cell of the ranges. */
  set: (
    result: ResultMeta,
    ranges: readonly CellRange[],
    value: CellValue,
  ) => void;
  /** One pasted value fills the selection; a TSV block lands from its top-left cell. */
  paste: (
    result: ResultMeta,
    ranges: readonly CellRange[],
    text: string,
  ) => void;
  deleteRows: (result: ResultMeta, ranges: readonly CellRange[]) => void;
  duplicateRows: (result: ResultMeta, ranges: readonly CellRange[]) => void;
};

type BulkContext = {
  io: EditIo;
  history: EditHistory;
  target: TableTargetRef;
  offset: number;
  /** Stage all edits as one undoable step. */
  stage: (edits: readonly StagedEdit[]) => void;
};

type Assignment = {
  cell: Cell;
  value: (column: TableColumnDto) => CellValue;
};
type Drafts = Map<number, StagedEdit>;

/** Pasted `NULL` restores SQL NULL where the column allows it; anything else is text. */
function pasted(text: string): Assignment['value'] {
  return (column) =>
    text === 'NULL' && column.nullable ? NULL_VALUE : textValue(text);
}

/** Context actions for the current grid selection. */
export function bulkEditing(context: BulkContext): BulkEditing {
  return {
    set: (result, ranges, value) => {
      assign(
        context,
        result,
        selectedCells(ranges).map((cell) => ({ cell, value: () => value })),
      );
    },
    paste: (result, ranges, text) => {
      assign(context, result, pasteAssignments(context, result, ranges, text));
    },
    deleteRows: (result, ranges) => {
      rows(context, result, selectedRows(ranges), (draft) => ({
        ...draft,
        kind: 'delete',
        values: {},
      }));
    },
    duplicateRows: (result, ranges) => {
      rows(context, result, selectedRows(ranges), (draft, info) =>
        duplicateRow(draft, info),
      );
    },
  };
}

function pasteAssignments(
  context: BulkContext,
  result: ResultMeta,
  ranges: readonly CellRange[],
  text: string,
): Assignment[] {
  const block = parseTsv(text);
  const single =
    block.length === 1 && block[0]?.length === 1 ? block[0][0] : undefined;
  if (single !== undefined)
    return selectedCells(ranges).map((cell) => ({
      cell,
      value: pasted(single),
    }));
  const start = origin(ranges);
  if (!start) return [];
  return pastePlan(block, start, {
    rows: result.rows + stagedInserts(context.history.current).length,
    columns: result.columns.length,
  }).map(({ cell, text: value }) => ({ cell, value: pasted(value) }));
}

function run(
  context: BulkContext,
  result: ResultMeta,
  rowIndexes: readonly number[],
  plan: (drafts: Drafts, info: EditInfoDto) => readonly StagedEdit[],
): void {
  const info = context.io.info;
  if (!writableResult(info, context.history, result) || !rowIndexes.length)
    return;
  void context.io.run(async () => {
    const drafts = await rowDrafts({ ...context, info, result }, rowIndexes);
    if (!context.io.alive.current) return;
    context.stage(plan(drafts, info));
  });
}

function rows(
  context: BulkContext,
  result: ResultMeta,
  rowIndexes: readonly number[],
  edit: (draft: StagedEdit, info: EditInfoDto) => StagedEdit,
): void {
  run(context, result, rowIndexes, (drafts, info) =>
    rowIndexes.flatMap((row) => {
      const draft = drafts.get(row);
      return draft ? [edit(draft, info)] : [];
    }),
  );
}

/** Validate every target cell first; a single problem stages nothing. */
function assign(
  context: BulkContext,
  result: ResultMeta,
  assignments: readonly Assignment[],
): void {
  const rowIndexes = [...new Set(assignments.map(({ cell }) => cell.row))];
  run(context, result, rowIndexes, (drafts, info) => {
    const problems: { row: string; message: string }[] = [];
    const edits: StagedEdit[] = [];
    for (const { cell, value } of assignments) {
      const draft = drafts.get(cell.row);
      const name = result.columns[cell.column]?.name;
      const column = info.columns.find((item) => item.name === name);
      if (!draft || !name || !column) continue;
      const staged = value(column);
      const locked = info.protected.includes(name) || draft.kind === 'delete';
      const error = cellError(column, locked, staged);
      if (error)
        problems.push({
          row: draft.number ? `Row ${String(draft.number)}` : 'New row',
          message: error,
        });
      else edits.push(fieldEdit(draft, name, staged));
    }
    if (problems.length) throw new Error(summarize(problems));
    return edits;
  });
}
