// SPDX-License-Identifier: Apache-2.0
import { memo } from 'react';
import type { KeyboardEvent } from 'react';
import { Badge } from '@openrows/ui';
import { columns, isEdited, statusTones } from './data/rows';
import type { OrderRow } from './data/rows';
import styles from './Grid.module.css';

type Props = {
  rows: readonly OrderRow[];
  selected: number;
  pending: boolean;
  onSelect: (index: number, row: OrderRow) => void;
};
/** Stable DOM mock grid; panel geometry never enters its props. */
export const DataGrid = memo(function DataGrid(
  props: Props,
): React.JSX.Element {
  function move(event: KeyboardEvent<HTMLDivElement>): void {
    if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const index =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? props.rows.length - 1
          : Math.max(
              0,
              Math.min(
                props.rows.length - 1,
                props.selected + (event.key === 'ArrowDown' ? 1 : -1),
              ),
            );
    const row = props.rows[index];
    if (!row) return;
    props.onSelect(index, row);
    const visible =
      event.currentTarget.querySelectorAll<HTMLElement>('[data-row]');
    const target = visible.item(index);
    if (typeof target.scrollIntoView === 'function')
      target.scrollIntoView({ block: 'nearest' });
  }
  return (
    <div
      role="grid"
      aria-label="Orders data"
      aria-rowcount={props.rows.length + 1}
      aria-colcount={8}
      tabIndex={0}
      className={styles.grid}
      onKeyDown={move}
      aria-activedescendant={`order-row-${String(props.selected)}`}
    >
      <div role="row" className={styles.header}>
        {['#', ...columns].map((column, index) => (
          <div
            role="columnheader"
            key={column}
            data-numeric={[0, 1, 2, 4].includes(index)}
          >
            {column}
          </div>
        ))}
      </div>
      {props.rows.map((row, index) => (
        <GridRow key={String(row[0])} row={row} index={index} owner={props} />
      ))}
    </div>
  );
});
function GridRow({
  row,
  index,
  owner,
}: {
  row: OrderRow;
  index: number;
  owner: Props;
}): React.JSX.Element {
  const inserted = typeof row[0] === 'string';
  const deleted = owner.pending && row[0] === 10427;
  return (
    <div
      role="row"
      id={`order-row-${String(index)}`}
      aria-rowindex={index + 2}
      aria-selected={index === owner.selected}
      aria-label={
        deleted
          ? 'Deleted order 10427'
          : inserted
            ? 'Inserted order'
            : `Order ${String(row[0])}`
      }
      data-row={index}
      data-inserted={inserted}
      data-deleted={deleted}
      className={styles.row}
      onClick={(event) => {
        owner.onSelect(index, row);
        event.currentTarget.parentElement?.focus({ preventScroll: true });
      }}
    >
      <div role="rowheader" data-numeric>
        {inserted ? '+' : deleted ? '−' : index + 1}
      </div>
      {row.map((value, column) => (
        <div
          role="gridcell"
          key={column}
          data-numeric={[0, 1, 3].includes(column)}
          data-null={value === null}
          data-edited={owner.pending && isEdited(index, column)}
          aria-label={
            owner.pending && isEdited(index, column)
              ? `Edited cell: ${String(value)}`
              : undefined
          }
        >
          {column === 2 ? (
            <Badge tone={statusTones[row[2]]}>{value}</Badge>
          ) : (
            (value ?? 'NULL')
          )}
        </div>
      ))}
    </div>
  );
}
