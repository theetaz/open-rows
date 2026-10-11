// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { StructureGrid } from '../table/structure/StructureGrid';
import type { MonitorTable } from './monitorData';
import { stateLabel } from './SourceState';
import styles from './Monitor.module.css';

type Row = MonitorTable['rows'][number];
/** Null-last numeric/text sorting, without mutating the source window. */
function sortedRows(
  table: MonitorTable,
  sort: { col: number; descending: boolean },
): Row[] {
  const key = table.columns[sort.col]?.name ?? '';
  return [...table.rows].sort((left, right) => {
    const a = left[key] ?? null;
    const b = right[key] ?? null;
    if (a === null) return b === null ? 0 : 1;
    if (b === null) return -1;
    const order =
      typeof a === 'number' && typeof b === 'number'
        ? a - b
        : String(a).localeCompare(String(b));
    return (sort.descending ? -1 : 1) * order;
  });
}
/** Reuse the Processes keyboard grid; page rows to bound DOM and repaint work. */
export function MonitorGrid(props: {
  label: string;
  table: MonitorTable;
  onRow?: (row: Row) => void;
}): React.JSX.Element {
  const [sort, setSort] = useState({ col: 0, descending: false });
  const [cursor, setCursor] = useState({ row: 0, col: 0 });
  const [page, setPage] = useState(0);
  const rows = sortedRows(props.table, sort).slice(
    page * 100,
    (page + 1) * 100,
  );
  const selected = rows[Math.min(cursor.row, Math.max(0, rows.length - 1))];
  return (
    <>
      <div className={styles.grid}>
        <StructureGrid
          label={props.label}
          columns={props.table.columns.map((column) => ({
            key: column.name,
            title: `${column.name.replaceAll('_', ' ')}${column.unit ? ` (${column.unit})` : ''}`,
            width: column.name === 'query' ? 420 : 150,
          }))}
          rows={rows.length}
          rowKey={(index) =>
            String(rows[index]?.[props.table.columns[0]?.name ?? ''] ?? index)
          }
          rowState={() => undefined}
          cell={(row, col) => {
            const value = rows[row]?.[props.table.columns[col]?.name ?? ''];
            return {
              content:
                value == null
                  ? 'Unknown'
                  : props.table.columns[col]?.name === 'interval_state'
                    ? stateLabel(String(value))
                    : String(value),
              muted: value == null,
            };
          }}
          cursor={{
            row: Math.min(cursor.row, Math.max(0, rows.length - 1)),
            col: cursor.col,
          }}
          onCursor={(next) => {
            setCursor(next);
            const row = rows[next.row];
            if (row) props.onRow?.(row);
          }}
          onKey={() => false}
          onEdit={() => {
            if (selected) props.onRow?.(selected);
          }}
          onMenu={() => undefined}
          sort={sort}
          onSort={(col) => {
            setSort({
              col,
              descending: sort.col === col ? !sort.descending : false,
            });
            setPage(0);
          }}
        />
      </div>
      <GridPages
        page={page}
        count={props.table.rows.length}
        partial={props.table.partial}
        onPage={setPage}
      />
    </>
  );
}

function GridPages(props: {
  page: number;
  count: number;
  partial: boolean;
  onPage: (page: number) => void;
}): React.JSX.Element {
  const shown = Math.min(100, Math.max(0, props.count - props.page * 100));
  return (
    <div className={styles.actions}>
      <button
        type="button"
        disabled={props.page === 0}
        onClick={() => {
          props.onPage(props.page - 1);
        }}
      >
        Previous rows
      </button>
      <span>
        {String(props.page * 100 + (shown ? 1 : 0))}–
        {String(props.page * 100 + shown)} of {String(props.count)}
        {props.partial ? ' · Partial source' : ''}
      </span>
      <button
        type="button"
        disabled={(props.page + 1) * 100 >= props.count}
        onClick={() => {
          props.onPage(props.page + 1);
        }}
      >
        Next rows
      </button>
    </div>
  );
}
