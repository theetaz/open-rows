// SPDX-License-Identifier: Apache-2.0
import type { GridProps, SelectionModel } from './model';

const HIDDEN =
  'position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);';
/** Shadow grid operations used by the canvas renderer. */
interface ShadowGrid {
  update: (selection: SelectionModel) => void;
  focus: () => void;
  /** Politely announce edit mode and staging outcomes. */
  announce: (message: string) => void;
  destroy: () => void;
}
/** One focused logical cell, not a million DOM rows; indices include the header row. */
export function shadowGrid(
  host: HTMLElement,
  props: GridProps,
  onMove?: (selection: SelectionModel) => void,
): ShadowGrid {
  const grid = document.createElement('div'),
    row = document.createElement('div'),
    cell = document.createElement('div'),
    status = document.createElement('div');
  initialize(grid, cell, props);
  row.setAttribute('role', 'row');
  cell.setAttribute('role', 'gridcell');
  status.setAttribute('role', 'status');
  status.style.cssText = HIDDEN;
  row.append(cell);
  grid.append(row);
  host.append(grid, status);
  const update = (selection: SelectionModel): void => {
    grid.setAttribute('aria-rowcount', String(props.source.rows + 1));
    row.setAttribute('aria-rowindex', String(selection.row + 2));
    cell.setAttribute('aria-colindex', String(selection.column + 1));
    cell.textContent = `${props.source.columns[selection.column]?.title ?? ''}: ${props.source.cell(selection.row, selection.column)?.text ?? 'Loading'}`;
  };
  grid.onkeydown = keyHandler(host, props, (selection) => {
    update(selection);
    onMove?.(selection);
  });
  const unsubscribe = props.source.subscribe(() => {
    update(props.selection);
  });
  update(props.selection);
  return {
    update,
    focus: () => {
      grid.focus({ preventScroll: true });
    },
    announce: (message) => {
      status.textContent = message;
    },
    destroy: () => {
      unsubscribe();
      grid.remove();
      status.remove();
    },
  };
}
/** Keyboard focus, range, edit and context-menu commands on the logical cell. */
function keyHandler(
  host: HTMLElement,
  props: GridProps,
  update: (selection: SelectionModel) => void,
): (event: KeyboardEvent) => void {
  return (event): void => {
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
    if (editKey(props, event)) {
      event.preventDefault();
      return;
    }
    const selection =
      event.key === ' '
        ? selectLine(props, event)
        : moveFocus(props.selection, event, props);
    if (!selection) return;
    event.preventDefault();
    props.selection = selection;
    update(selection);
    props.onSelection(selection);
  };
}
/** Space selects the focused row; Ctrl/Cmd+Space selects the focused column. */
function selectLine(
  props: GridProps,
  event: KeyboardEvent,
): SelectionModel | undefined {
  if (props.source.rows === 0) return undefined;
  const { row, column } = props.selection;
  const range =
    event.ctrlKey || event.metaKey
      ? { r0: 0, r1: props.source.rows - 1, c0: column, c1: column }
      : {
          r0: row,
          r1: row,
          c0: props.rowHeaderColumn === 0 ? 1 : 0,
          c1: props.source.columns.length - 1,
        };
  return { row, column, ranges: [range] };
}
/** Enter, F2, Space on a stepper column, or a printable key that seeds the editor. */
function editKey(props: GridProps, event: KeyboardEvent): boolean {
  if (props.source.rows === 0 || !props.onEdit || event.isComposing)
    return false;
  const { row, column } = props.selection;
  const kind = props.source.columns[column]?.kind;
  const stepper = kind === 'boolean' || kind === 'enum';
  const trigger = editTrigger(event, stepper);
  if (trigger === 'open') props.onEdit({ row, column });
  else if (trigger === 'seed') props.onEdit({ row, column }, event.key);
  return trigger !== undefined;
}
/** Stepper columns open their value menu instead of taking typed text. */
function editTrigger(
  event: KeyboardEvent,
  stepper: boolean,
): 'open' | 'seed' | undefined {
  const plain = !event.metaKey && !event.ctrlKey && !event.altKey;
  if (event.key === 'Enter' || event.key === 'F2') return 'open';
  if (event.key === ' ')
    return stepper && plain && !event.shiftKey ? 'open' : undefined;
  if (event.key.length !== 1 || !plain) return undefined;
  return stepper ? 'open' : 'seed';
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
  grid.style.cssText = HIDDEN;
  grid.tabIndex = 0;
  cell.id = `grid-focus-${String(++sequence)}`;
  grid.setAttribute('aria-activedescendant', cell.id);
}
