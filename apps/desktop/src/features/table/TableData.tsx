// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useRef, useState } from 'react';
import { Button } from '@openrows/ui';
import { isActive } from '../query/run';
import type { ResultMeta } from '../query/run';
import type { TableTabState } from './useTableTab';
import type { RowEditing } from './useRowEditing';
import type { ContextTarget } from './contextMenuActions';
import type { GridSelection } from './gridSelection';
import type { TableLayout } from './useTableLayout';
import type { DataTools } from './useDataTools';
import { RowForm } from './RowForm';
import { tableSettings } from '../settings/tableSettings';
import styles from './Table.module.css';
const TableGrid = lazy(() =>
  import('./TableGrid').then((module) => ({ default: module.TableGrid })),
);

type Props = {
  state: TableTabState;
  editing: RowEditing;
  /** The table's column widths, order and pins. */
  columns: TableLayout;
  onRow: (result: ResultMeta, row: number) => void;
  onContextMenu: (result: ResultMeta, target: ContextTarget) => void;
  /** The grid's selection changed, for table commands. */
  onSelection: (selection: GridSelection) => void;
  /** Keys, grid position and the form row. */
  tools: DataTools;
  /** A session is open: rows stage. */
  ready: boolean;
  /** Table name, for the form's caption. */
  table: string;
};

/** The last finished page and the sort it was read with. */
type Shown = { result: ResultMeta; sort: TableTabState['view']['sort'] };

/** The page's grid, or why there is none yet (loading, error, no rows). */
export function TableData(props: Props): React.JSX.Element {
  const { state, editing, tools } = props;
  const { entry, result, view } = state;
  const fitted = useRef(new Map<string, number>());
  // While a re-sort, filter or page loads, keep showing the last rows instead of a loading note.
  const [shown, setShown] = useState<Shown | null>(null);
  if (result?.done && result !== shown?.result)
    setShown({ result, sort: view.sort });
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
  if (!result?.done && shown && isActive(entry.run))
    return (
      <Grid
        {...props}
        result={shown.result}
        sort={shown.sort}
        fitted={fitted}
      />
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
  if (view.form)
    return (
      <RowForm
        table={props.table}
        result={result}
        editing={editing}
        ready={props.ready}
        offset={view.offset}
        row={Math.min(tools.formRow, Math.max(0, result.rows - 1))}
        onRow={tools.setFormRow}
        onClose={tools.actions.toggleForm}
      />
    );
  return <Grid {...props} result={result} sort={view.sort} fitted={fitted} />;
}

/** The page's canvas grid, mounted lazily once per page. */
function Grid({
  state,
  onRow,
  editing,
  columns,
  onContextMenu,
  onSelection,
  tools,
  result,
  sort,
  fitted,
}: Props & {
  result: ResultMeta;
  sort: Shown['sort'];
  fitted: React.RefObject<Map<string, number>>;
}): React.JSX.Element {
  const { view, actions } = state;
  const defaults = tableSettings.useValue();
  return (
    <Suspense fallback={null}>
      <TableGrid
        key={result.id}
        result={result}
        fitted={fitted}
        sort={sort}
        editing={editing}
        layout={columns.layout}
        onLayout={columns.change}
        copyTarget={columns.grid.copyTarget}
        links={tools.jumps.columns}
        onLink={(cell) => {
          tools.jumps.openCell(result, cell);
        }}
        position={tools.position}
        onContextMenu={(target) => {
          onContextMenu(result, target);
        }}
        offset={view.offset}
        rowNumbers={view.rowNumbers}
        striped={view.striped ?? defaults.striped}
        onHeader={(column) => {
          actions.sort(result, column);
        }}
        onRow={(row) => {
          onRow(result, row);
        }}
        onSelection={(cell, ranges) => {
          onSelection({ result, cell, ranges });
        }}
      />
    </Suspense>
  );
}
