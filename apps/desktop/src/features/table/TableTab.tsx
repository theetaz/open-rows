// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Button } from '@openrows/ui';
import type { ResultMeta } from '../query/run';
import type { TableObject } from '../query/tabs';
import { canEdit, EditToolbar, editingNote } from './EditToolbar';
import { RowEditSheet } from './RowEditSheet';
import { useRowEditing } from './useRowEditing';
import { useTableContext } from './TableContextMenu';
import { ColumnsPanel } from './ColumnsPanel';
import { FilterBar } from './FilterBar';
import { StructureView } from './StructureView';
import { TableData } from './TableData';
import { TableFooter } from './TableFooter';
import { readRow } from './row';
import type { SelectedRow } from './row';
import { useTableKeys } from './useTableKeys';
import { useTableTab } from './useTableTab';
import type { TableViews } from './useTableViews';
import styles from './Table.module.css';

const noKeys: readonly string[] = [];

type TableTabProps = {
  tabId: string;
  table: TableObject;
  profileId: string;
  /** The session pages run on, or `null` when they cannot run here. */
  sessionId: string | null;
  /** Why rows cannot load even though the connection is open. */
  notice: string | null;
  views: TableViews;
  onRow: (row: SelectedRow) => void;
  onConnect: () => void;
};

/** A TablePlus-style table view: filters, paged canvas grid, structure, footer. */
export function TableTab(props: TableTabProps): React.JSX.Element {
  const { table, sessionId } = props;
  const root = useRef<HTMLElement>(null);
  const rowSequence = useRef(0);
  const state = useTableTab(props);
  const { view, actions, columns } = state;
  useTableKeys(root, actions);
  const editing = useRowEditing({
    ...props,
    target: state.query.target,
    offset: view.offset,
  });
  const showRow = (result: ResultMeta, row: number): void => {
    rowHandler({
      table,
      view,
      editing,
      onRow: props.onRow,
      sequence: rowSequence,
    })(result, row);
  };
  const context = useTableContext({
    editing,
    changeView: (update) => {
      props.views.setView(props.tabId, update);
    },
  });
  return (
    <section
      ref={root}
      className={styles.table}
      aria-label={`${table.name} table`}
    >
      <Banner {...props} />
      {view.mode === 'data' && view.filtersOpen && (
        <FilterBar
          view={view}
          columns={columns}
          query={state.query}
          actions={actions}
        />
      )}
      {view.mode === 'data' ? (
        sessionId && (
          <TableData
            state={state}
            onRow={showRow}
            onEdit={editing.open}
            onContextMenu={context.show}
            changes={editing.history.current}
            editKeys={editing.info?.key ?? noKeys}
          />
        )
      ) : (
        <StructureView structure={state.entry.structure} show={view.mode} />
      )}
      {view.columnsOpen && (
        <ColumnsPanel
          columns={columns}
          hidden={view.hidden}
          actions={actions}
        />
      )}
      {context.menu}
      <EditingFooter
        state={state}
        editing={editing}
        ready={sessionId !== null}
      />
    </section>
  );
}

/** Staged rows, the pending-changes bar, edit errors and the table footer. */
function EditingFooter({
  state,
  editing,
  ready,
}: {
  state: ReturnType<typeof useTableTab>;
  editing: ReturnType<typeof useRowEditing>;
  ready: boolean;
}): React.JSX.Element {
  const data = state.view.mode === 'data';
  return (
    <>
      {data && <EditToolbar editing={editing} ready={ready} />}
      {editing.error && !editing.review && (
        <p role="alert" className={styles.banner}>
          {editing.error}
        </p>
      )}
      <RowEditSheet editing={editing} />
      <TableFooter
        state={state}
        ready={ready}
        onInsert={data && canEdit(editing, ready) ? editing.insert : undefined}
        insertTitle={editingNote(editing)}
      />
    </>
  );
}

/** Why rows are not shown: another database's tab, or no open session. */
function Banner(props: TableTabProps): React.JSX.Element | null {
  if (props.notice)
    return (
      <p className={styles.banner} role="note">
        {props.notice}
      </p>
    );
  if (props.sessionId !== null) return null;
  return (
    <p className={styles.banner}>
      Not connected.{' '}
      <Button size="sm" variant="ghost" onClick={props.onConnect}>
        Connect to browse rows
      </Button>
    </p>
  );
}

function rowHandler({
  table,
  view,
  editing,
  onRow,
  sequence,
}: {
  table: TableObject;
  view: import('./model').TableView;
  editing: import('./useRowEditing').RowEditing;
  onRow: TableTabProps['onRow'];
  sequence: React.RefObject<number>;
}) {
  return (result: ResultMeta, row: number): void => {
    const ticket = ++sequence.current;
    readRow(result, row).then(
      (fields) => {
        if (ticket !== sequence.current) return;
        onRow({
          table: table.name,
          number: view.offset + row + 1,
          fields,
          ...(editing.info && !editing.info.readOnly && editing.info.key.length
            ? {
                onEdit: (name: string) => {
                  const column = result.columns.findIndex(
                    (item) => item.name === name,
                  );
                  if (column >= 0) editing.open(result, { row, column });
                },
              }
            : {}),
        });
      },
      (error: unknown) => {
        onRow({
          table: table.name,
          number: view.offset + row + 1,
          fields: [],
          error: error instanceof Error ? error.message : String(error),
        });
      },
    );
  };
}
