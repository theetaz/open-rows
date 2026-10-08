// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import styles from './Structure.module.css';

/** A grid column: header title and fixed width in px. */
export type GridColumnDef = { key: string; title: string; width: number };

/** The focused cell. */
export type Cursor = { row: number; col: number };

/** A rendered cell: content, muted (NULL/EMPTY) and changed markers. */
export type CellView = {
  content: ReactNode;
  muted?: boolean;
  changed?: boolean;
};

type Props = {
  ref?: React.Ref<HTMLDivElement>;
  label: string;
  columns: readonly GridColumnDef[];
  rows: number;
  rowKey: (row: number) => string;
  rowState: (row: number) => 'dropped' | 'added' | undefined;
  cell: (row: number, col: number) => CellView;
  cursor: Cursor;
  onCursor: (cursor: Cursor) => void;
  /** Keys the grid does not handle itself (Enter, Delete, …); `true` when used. */
  onKey: (event: KeyboardEvent<HTMLElement>, cursor: Cursor) => boolean;
  onEdit: (cursor: Cursor) => void;
  onMenu: (row: number, at: { x: number; y: number }) => void;
  /** Click on the empty row after the last one (e.g. a new column). */
  onFiller?: (() => void) | undefined;
  fillerLabel?: string;
};

const move: Record<string, [number, number]> = {
  ArrowUp: [-1, 0],
  ArrowDown: [1, 0],
  ArrowLeft: [0, -1],
  ArrowRight: [0, 1],
};

function next(cursor: Cursor, key: string, size: Cursor): Cursor | null {
  const step = move[key];
  if (step)
    return {
      row: Math.min(size.row - 1, Math.max(0, cursor.row + step[0])),
      col: Math.min(size.col - 1, Math.max(0, cursor.col + step[1])),
    };
  if (key === 'Home') return { ...cursor, col: 0 };
  if (key === 'End') return { ...cursor, col: size.col - 1 };
  return null;
}

/** Keyboard-navigable table styled like the canvas Data grid (role grid). */
export function StructureGrid({ ref, ...props }: Props): React.JSX.Element {
  const id = useId();
  const cellId = (row: number, col: number): string =>
    `${id}-${String(row)}-${String(col)}`;
  const keyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (event.target !== event.currentTarget) return;
    if (props.onKey(event, props.cursor)) return;
    const size = { row: props.rows, col: props.columns.length };
    const target = props.rows ? next(props.cursor, event.key, size) : null;
    if (target) {
      event.preventDefault();
      props.onCursor(target);
      const node = document.getElementById(cellId(target.row, target.col));
      if (typeof node?.scrollIntoView === 'function')
        node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    } else if (event.key === 'Enter' || event.key === 'F2') {
      event.preventDefault();
      if (props.rows) props.onEdit(props.cursor);
    } else if (
      event.key === 'ContextMenu' ||
      (event.shiftKey && event.key === 'F10')
    ) {
      event.preventDefault();
      const box = document
        .getElementById(cellId(props.cursor.row, props.cursor.col))
        ?.getBoundingClientRect();
      if (box) props.onMenu(props.cursor.row, { x: box.left, y: box.bottom });
    }
  };
  return (
    <div
      ref={ref}
      className={styles.scroll}
      role="grid"
      aria-label={props.label}
      aria-rowcount={props.rows + 1}
      aria-activedescendant={
        props.rows ? cellId(props.cursor.row, props.cursor.col) : undefined
      }
      tabIndex={0}
      onKeyDown={keyDown}
    >
      <table className={styles.grid} role="presentation">
        <colgroup>
          {props.columns.map((column) => (
            <col key={column.key} style={{ width: column.width }} />
          ))}
        </colgroup>
        <thead role="rowgroup">
          <tr role="row">
            {props.columns.map((column) => (
              <th key={column.key} role="columnheader" scope="col">
                {column.title}
              </th>
            ))}
          </tr>
        </thead>
        <Body {...props} cellId={cellId} />
      </table>
      <div
        className={styles.filler}
        data-odd={(props.rows + (props.onFiller ? 1 : 0)) % 2 === 1}
      />
    </div>
  );
}

function Body(
  props: Omit<Props, 'ref'> & { cellId: (row: number, col: number) => string },
): React.JSX.Element {
  return (
    <tbody role="rowgroup">
      {Array.from({ length: props.rows }, (_, row) => (
        <tr
          key={props.rowKey(row)}
          role="row"
          data-state={props.rowState(row)}
          onContextMenu={(event) => {
            event.preventDefault();
            props.onCursor({ row, col: props.cursor.col });
            props.onMenu(row, { x: event.clientX, y: event.clientY });
          }}
        >
          {props.columns.map((column, col) => {
            const view = props.cell(row, col);
            return (
              <td
                key={column.key}
                id={props.cellId(row, col)}
                role="gridcell"
                data-number={col === 0 ? true : undefined}
                data-muted={view.muted}
                data-changed={view.changed}
                data-active={
                  props.cursor.row === row && props.cursor.col === col
                }
                onMouseDown={() => {
                  props.onCursor({ row, col });
                }}
                onDoubleClick={() => {
                  props.onEdit({ row, col });
                }}
              >
                {view.content}
              </td>
            );
          })}
        </tr>
      ))}
      {props.onFiller && (
        <tr role="row" data-filler>
          <td
            role="gridcell"
            colSpan={props.columns.length}
            onClick={props.onFiller}
          >
            {props.fillerLabel}
          </td>
        </tr>
      )}
    </tbody>
  );
}
