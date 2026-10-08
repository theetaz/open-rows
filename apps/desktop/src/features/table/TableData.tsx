// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Button } from '@openrows/ui';
import { isActive } from '../query/run';
import type { ResultMeta } from '../query/run';
import type { TableTabState } from './useTableTab';
import type { RowEditing } from './useRowEditing';
import type { ContextTarget } from './contextMenuActions';
import styles from './Table.module.css';
const TableGrid = lazy(() =>
  import('./TableGrid').then((module) => ({ default: module.TableGrid })),
);

type Props = {
  state: TableTabState;
  editing: RowEditing;
  onRow: (result: ResultMeta, row: number) => void;
  onContextMenu: (result: ResultMeta, target: ContextTarget) => void;
};

/** The page's grid, or why there is none yet (loading, error, no rows). */
export function TableData({
  state,
  onRow,
  editing,
  onContextMenu,
}: Props): React.JSX.Element {
  const { entry, result, view, actions } = state;
  const errors = entry.run.messages.filter((item) => item.level === 'error');
  if (errors.length)
    return (
      <div role="alert" className={styles.error}>
        {errors.map((item, index) => (
          <p key={index}>
            {item.code ? `${item.code}: ` : ''}
            {item.text}
          </p>
        ))}
      </div>
    );
  if (!result?.done)
    return (
      <div role="status" className={styles.note}>
        {isActive(entry.run) ? (
          <>
            Loading rows…{' '}
            <Button size="sm" variant="ghost" onClick={state.cancelPage}>
              Cancel
            </Button>
          </>
        ) : (
          'No rows loaded'
        )}
      </div>
    );
  return (
    <Suspense fallback={null}>
      <TableGrid
        key={result.id}
        result={result}
        sort={view.sort}
        editing={editing}
        onContextMenu={(target) => {
          onContextMenu(result, target);
        }}
        offset={view.offset}
        rowNumbers={view.rowNumbers}
        striped={view.striped}
        onHeader={(column) => {
          actions.sort(result, column);
        }}
        onRow={(row) => {
          onRow(result, row);
        }}
      />
    </Suspense>
  );
}
