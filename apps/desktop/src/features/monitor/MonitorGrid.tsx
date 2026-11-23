// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { StructureGrid } from '../table/structure/StructureGrid';
import type { MonitorTable } from './monitorData';
import { stateLabel } from './SourceState';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

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
/** The grid's height in rows: at least 4, at most 14, then it scrolls. */
export function gridRows(count: number): React.CSSProperties {
  return {
    '--grid-rows': String(Math.min(14, Math.max(4, count))),
  } as React.CSSProperties;
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
      <div className={styles.grid} style={gridRows(rows.length)}>
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
                  ? t('unknown')
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
      <Button
        size="sm"
        variant="ghost"
        disabled={props.page === 0}
        onClick={() => {
          props.onPage(props.page - 1);
        }}
      >
        {t('previousRows')}
      </Button>
      <span>
        {props.partial
          ? t('valueValue2Count', {
              value: String(props.page * 100 + (shown ? 1 : 0)),
              value2: String(props.page * 100 + shown),
              count: String(props.count),
            })
          : t('valueValue2Count2', {
              value: String(props.page * 100 + (shown ? 1 : 0)),
              value2: String(props.page * 100 + shown),
              count: String(props.count),
            })}
      </span>
      <Button
        size="sm"
        variant="ghost"
        disabled={(props.page + 1) * 100 >= props.count}
        onClick={() => {
          props.onPage(props.page + 1);
        }}
      >
        {t('nextRows')}
      </Button>
    </div>
  );
}
