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
    if (
      event.key === 'ContextMenu' ||
      (event.key === 'F10' && event.shiftKey)
    ) {
      event.preventDefault();
      const bounds = host.getBoundingClientRect();
      props.onContextMenu?.({
        ...props.selection,
        kind: 'cell',
        x: bounds.left + 48,
        y: bounds.top + 48,
      });
      return;
    }
    if (event.key === 'Enter' && props.source.rows > 0) {
      event.preventDefault();
      props.onEdit?.({
        row: props.selection.row,
        column: props.selection.column,
      });
      return;
    }
    if (event.key === ' ' && props.source.rows > 0) {
      event.preventDefault();
      const { row, column } = props.selection;
      const ranges =
        event.ctrlKey || event.metaKey
          ? [{ r0: 0, r1: props.source.rows - 1, c0: column, c1: column }]
          : [
              {
                r0: row,
                r1: row,
                c0: props.rowHeaderColumn === 0 ? 1 : 0,
                c1: props.source.columns.length - 1,
              },
            ];
      props.selection = { row, column, ranges };
      update(props.selection);
      props.onSelection(props.selection);
      return;
    }
    const selection = moveFocus(props.selection, event, props);
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
  event: KeyboardEvent,
  props: GridProps,
): SelectionModel | undefined {
  const delta = {
    ArrowUp: [-1, 0],
    ArrowDown: [1, 0],
    ArrowLeft: [0, -1],
    ArrowRight: [0, 1],
  }[event.key];
  if (!delta) return undefined;
  const row = Math.max(
    0,
    Math.min(props.source.rows - 1, current.row + (delta[0] ?? 0)),
  );
  const column = Math.max(
    props.rowHeaderColumn === 0 ? 1 : 0,
    Math.min(props.source.columns.length - 1, current.column + (delta[1] ?? 0)),
  );
  const anchor = event.shiftKey ? (current.anchor ?? current) : { row, column };
  return {
    row,
    column,
    anchor: { row: anchor.row, column: anchor.column },
    ranges: [
      {
        r0: Math.min(anchor.row, row),
        r1: Math.max(anchor.row, row),
        c0: Math.min(anchor.column, column),
        c1: Math.max(anchor.column, column),
      },
    ],
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
