// SPDX-License-Identifier: Apache-2.0
import type { CellRange } from '@openrows/grid';

/** One grid cell in result (data) coordinates. */
export type Cell = { row: number; column: number };

/** Grid ranges in result coordinates: the row-number column, if shown, is dropped. */
export function dataRanges(
  ranges: readonly CellRange[],
  rowNumbers: boolean,
): CellRange[] {
  const shift = rowNumbers ? 1 : 0;
  return ranges
    .map((range) => ({
      ...range,
      c0: Math.max(0, range.c0 - shift),
      c1: range.c1 - shift,
    }))
    .filter((range) => range.c1 >= range.c0 && range.r1 >= range.r0);
}

/** Every distinct cell of the ranges, row-major; the cap keeps the staging bounded. */
export function selectedCells(
  ranges: readonly CellRange[],
  cap = 100_000,
): Cell[] {
  const seen = new Set<string>();
  const cells: Cell[] = [];
  for (const range of ranges)
    for (let row = range.r0; row <= range.r1; row++)
      for (let column = range.c0; column <= range.c1; column++) {
        const key = `${String(row)}:${String(column)}`;
        if (seen.has(key)) continue;
        seen.add(key);
        if (cells.length >= cap) return cells;
        cells.push({ row, column });
      }
  return cells;
}

/** Distinct rows of the ranges, ascending. */
export function selectedRows(ranges: readonly CellRange[]): number[] {
  const rows = new Set<number>();
  for (const range of ranges)
    for (let row = range.r0; row <= range.r1; row++) rows.add(row);
  return [...rows].sort((a, b) => a - b);
}

/** Whether `cell` lies inside any of the ranges. */
export function inRanges(ranges: readonly CellRange[], cell: Cell): boolean {
  return ranges.some(
    (range) =>
      cell.row >= range.r0 &&
      cell.row <= range.r1 &&
      cell.column >= range.c0 &&
      cell.column <= range.c1,
  );
}

/** The top-left cell of the first range; where a pasted block starts. */
export function origin(ranges: readonly CellRange[]): Cell | null {
  const first = ranges[0];
  return first ? { row: first.r0, column: first.c0 } : null;
}

/**
 * Parse clipboard text as TSV: rows split on line breaks (a trailing one is
 * ignored), cells on tabs; a `"…"` cell may hold tabs, line breaks and `""`.
 */
export function parseTsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let index = 0;
  while (index < text.length) {
    const char = text[index] ?? '';
    if (char === '"' && cell === '') {
      const quoted = readQuoted(text, index + 1);
      cell = quoted.value;
      index = quoted.end;
      continue;
    }
    if (char === '\t') {
      row.push(cell);
      cell = '';
    } else if (char === '\n' || char === '\r') {
      if (char === '\r' && text[index + 1] === '\n') index += 1;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = '';
    } else cell += char;
    index += 1;
  }
  if (cell !== '' || row.length) rows.push([...row, cell]);
  return rows;
}

/** The content of a quoted cell starting after its opening quote, and where it ends. */
function readQuoted(
  text: string,
  from: number,
): { value: string; end: number } {
  let value = '';
  let index = from;
  while (index < text.length) {
    const char = text[index] ?? '';
    if (char === '"') {
      if (text[index + 1] !== '"') return { value, end: index + 1 };
      value += '"';
      index += 2;
    } else {
      value += char;
      index += 1;
    }
  }
  return { value, end: index };
}

/** Cells and the text each receives when `block` is pasted from `start`, within `bounds`. */
export function pastePlan(
  block: readonly (readonly string[])[],
  start: Cell,
  bounds: { rows: number; columns: number },
): { cell: Cell; text: string }[] {
  const plan: { cell: Cell; text: string }[] = [];
  block.forEach((line, r) => {
    const row = start.row + r;
    if (row >= bounds.rows) return;
    line.forEach((text, c) => {
      const column = start.column + c;
      if (column < bounds.columns) plan.push({ cell: { row, column }, text });
    });
  });
  return plan;
}
