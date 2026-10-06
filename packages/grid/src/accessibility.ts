// SPDX-License-Identifier: Apache-2.0
import type { GridProps, SelectionModel } from './model';

/** One focused logical cell, not a million DOM rows; indices include the header row. */
export function shadowGrid(
  host: HTMLElement,
  props: GridProps,
): { update: (selection: SelectionModel) => void; destroy: () => void } {
  const grid = document.createElement('div'),
    row = document.createElement('div'),
    cell = document.createElement('div');
  initialize(grid, cell, props);
  row.setAttribute('role', 'row');
  cell.setAttribute('role', 'gridcell');
  row.append(cell);
  grid.append(row);
  host.append(grid);
  const update = (selection: SelectionModel): void => {
    row.setAttribute('aria-rowindex', String(selection.row + 2));
    cell.setAttribute('aria-colindex', String(selection.column + 1));
    cell.textContent = `${props.source.columns[selection.column]?.title ?? ''}: ${props.source.cell(selection.row, selection.column)?.text ?? 'Loading'}`;
  };
  grid.onkeydown = (event): void => {
    const selection = moveFocus(props.selection, event.key, props);
    if (!selection) return;
    event.preventDefault();
    props.selection = selection;
    update(selection);
    props.onSelection(selection);
  };
  const unsubscribe = props.source.subscribe(() => {
    update(props.selection);
  });
  update(props.selection);
  return {
    update,
    destroy: () => {
      unsubscribe();
      grid.remove();
    },
  };
}
let sequence = 0;
function moveFocus(
  current: SelectionModel,
  key: string,
  props: GridProps,
): SelectionModel | undefined {
  const delta = {
    ArrowUp: [-1, 0],
    ArrowDown: [1, 0],
    ArrowLeft: [0, -1],
    ArrowRight: [0, 1],
  }[key];
  if (!delta) return undefined;
  return {
    ...current,
    row: Math.max(
      0,
      Math.min(props.source.rows - 1, current.row + (delta[0] ?? 0)),
    ),
    column: Math.max(
      0,
      Math.min(
        props.source.columns.length - 1,
        current.column + (delta[1] ?? 0),
      ),
    ),
  };
}

function initialize(
  grid: HTMLElement,
  cell: HTMLElement,
  props: GridProps,
): void {
  grid.setAttribute('role', 'grid');
  grid.setAttribute('aria-label', 'Result grid');
  grid.setAttribute('aria-rowcount', String(props.source.rows + 1));
  grid.setAttribute('aria-colcount', String(props.source.columns.length));
  grid.style.cssText =
    'position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);';
  grid.tabIndex = 0;
  cell.id = `grid-focus-${String(++sequence)}`;
  grid.setAttribute('aria-activedescendant', cell.id);
}
