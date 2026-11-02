// SPDX-License-Identifier: Apache-2.0
import { keyMove, leaveGrid } from './keyboard';
import type { KeyMove } from './keyboard';
import type { GridProps, SelectionModel } from './model';
import { announcer } from './announce';
import { shadowRows } from './shadowRows';

const HIDDEN =
  'position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);';
/** Shadow grid operations used by the canvas renderer. */
interface ShadowGrid {
  update: (selection: SelectionModel) => void;
  focus: () => void;
  /** Politely announce edit mode and staging outcomes at once. */
  announce: (message: string) => void;
  destroy: () => void;
}
/** Viewport hooks for keyboard moves, supplied by the renderer. */
export interface GridNavigation {
  /** Whole data rows that fit below the header: one Page Up/Down step. */
  pageRows: () => number;
  /** Scroll by `scrollRows`, then reveal the focused cell. */
  moved: (selection: SelectionModel, scrollRows: number) => void;
}
/**
 * One focused logical cell with its column header and neighbouring rows, not a
 * million DOM rows; indices include the header row.
 */
export function shadowGrid(
  host: HTMLElement,
  props: GridProps,
  navigation: GridNavigation,
): ShadowGrid {
  const grid = document.createElement('div'),
    status = document.createElement('div');
  initialize(grid, props);
  const rows = shadowRows(grid, props);
  rows.active.id = `grid-focus-${String(++sequence)}`;
  grid.setAttribute('aria-activedescendant', rows.active.id);
  status.setAttribute('role', 'status');
  status.style.cssText = HIDDEN;
  host.append(grid, status);
  const announcements = announcer(status, props, props.selection);
  const update = (selection: SelectionModel): void => {
    grid.setAttribute('aria-rowcount', String(props.source.rows + 1));
    grid.setAttribute('aria-colcount', String(props.source.columns.length));
    rows.render(selection);
    announcements.selection(selection);
  };
  grid.onkeydown = keyHandler(host, props, {
    pageRows: navigation.pageRows,
    moved: (selection, scrollRows) => {
      update(selection);
      navigation.moved(selection, scrollRows);
    },
  });
  const unsubscribe = props.source.subscribe(() => {
    update(props.selection);
    announcements.refresh(props.selection);
  });
  update(props.selection);
  return {
    update,
    focus: () => {
      grid.focus({ preventScroll: true });
    },
    announce: announcements.message,
    destroy: () => {
      unsubscribe();
      announcements.destroy();
      grid.remove();
      status.remove();
    },
  };
}
/** Keyboard focus, range, edit and context-menu commands on the logical cell. */
function keyHandler(
  host: HTMLElement,
  props: GridProps,
  navigation: GridNavigation,
): (event: KeyboardEvent) => void {
  return (event): void => {
    if (event.key === 'Tab' && event.ctrlKey && !event.metaKey) {
      if (leaveGrid(host, event.shiftKey)) event.preventDefault();
      return;
    }
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
    const move = keyCommand(props, event, navigation.pageRows());
    if (!move) return;
    event.preventDefault();
    props.selection = move.selection;
    navigation.moved(move.selection, move.scrollRows);
    props.onSelection(move.selection);
  };
}
/** Space selects a line; every other navigation key moves the focused cell. */
function keyCommand(
  props: GridProps,
  event: KeyboardEvent,
  pageRows: number,
): KeyMove | undefined {
  if (event.key !== ' ')
    return keyMove(props.selection, event, props, pageRows);
  const selection = selectLine(props, event);
  return selection && { selection, scrollRows: 0 };
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

function initialize(grid: HTMLElement, props: GridProps): void {
  grid.setAttribute('role', 'grid');
  grid.setAttribute('aria-label', 'Result grid');
  grid.setAttribute('aria-multiselectable', 'true');
  grid.setAttribute('aria-rowcount', String(props.source.rows + 1));
  grid.setAttribute('aria-colcount', String(props.source.columns.length));
  grid.setAttribute(
    'aria-description',
    'Tab moves between cells; Control-Tab leaves the grid.',
  );
  grid.style.cssText = HIDDEN;
  grid.tabIndex = 0;
}
