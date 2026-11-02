// SPDX-License-Identifier: Apache-2.0
import type { GridProps, SelectionModel } from './model';

/** Previous, focused and next model rows around the active cell. */
const NEIGHBOURS = [-1, 0, 1] as const;

/** Header row plus three body rows: a bounded DOM whatever the row count. */
interface ShadowRows {
  /** The focused cell, referenced by the grid's `aria-activedescendant`. */
  readonly active: HTMLElement;
  render: (selection: SelectionModel) => void;
}

interface BodyRow {
  row: HTMLElement;
  header: HTMLElement;
  cell: HTMLElement;
}

/**
 * Build the column header and the focused row between its neighbours, so a
 * screen reader reads the active cell with its column header, row header and
 * neighbouring values. Indices include the header row (aria-rowindex 1).
 */
export function shadowRows(grid: HTMLElement, props: GridProps): ShadowRows {
  const headerRow = element('row'),
    columnHeader = element('columnheader');
  headerRow.setAttribute('aria-rowindex', '1');
  headerRow.append(columnHeader);
  const body = NEIGHBOURS.map((): BodyRow => {
    const row = element('row'),
      header = element('rowheader'),
      cell = element('gridcell');
    row.append(header, cell);
    return { row, header, cell };
  });
  grid.append(headerRow, ...body.map(({ row }) => row));
  const active = body[1]?.cell ?? element('gridcell');
  return {
    active,
    render: (selection) => {
      const column = props.source.columns[selection.column];
      columnHeader.setAttribute('aria-colindex', String(selection.column + 1));
      columnHeader.textContent = column?.title ?? '';
      body.forEach((parts, index) => {
        renderRow(props, parts, selection, NEIGHBOURS[index] ?? 0);
      });
    },
  };
}

/** One body row, hidden when the neighbour lies outside the result. */
function renderRow(
  props: GridProps,
  { row, header, cell }: BodyRow,
  selection: SelectionModel,
  offset: number,
): void {
  const model = selection.row + offset;
  row.hidden = model < 0 || model >= props.source.rows;
  if (row.hidden) return;
  row.setAttribute('aria-rowindex', String(model + 2));
  const gutter = props.rowHeaderColumn;
  // The focused gutter cell is the row header itself; never repeat it.
  header.hidden = gutter === undefined || gutter === selection.column;
  if (gutter !== undefined) {
    header.setAttribute('aria-colindex', String(gutter + 1));
    header.textContent = rowLabel(props, model);
  }
  cell.setAttribute(
    'role',
    gutter === selection.column ? 'rowheader' : 'gridcell',
  );
  cell.setAttribute('aria-colindex', String(selection.column + 1));
  cell.setAttribute('aria-selected', String(selected(selection, model)));
  cell.textContent = cellText(props, model, selection.column);
}

/** The row-number gutter's label (it may include a page offset), else the 1-based row. */
export function rowLabel(props: GridProps, row: number): string {
  const gutter = props.rowHeaderColumn;
  const label =
    gutter === undefined ? undefined : props.source.cell(row, gutter)?.text;
  return label === undefined || label === '' ? String(row + 1) : label;
}

/** Displayed text, or `Loading` until the tile arrives. */
export function cellText(
  props: GridProps,
  row: number,
  column: number,
): string {
  return props.source.cell(row, column)?.text ?? 'Loading';
}

/** Whether a model cell in the focused column lies inside any selected range. */
function selected(selection: SelectionModel, row: number): boolean {
  const { column } = selection;
  return selection.ranges.some(
    (range) =>
      row >= range.r0 &&
      row <= range.r1 &&
      column >= range.c0 &&
      column <= range.c1,
  );
}

function element(role: string): HTMLElement {
  const node = document.createElement('div');
  node.setAttribute('role', role);
  return node;
}
