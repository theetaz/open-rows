// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Button } from '@openrows/ui';
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
import type { ShowRow } from './row';
import { useSelectedRow } from './useSelectedRow';
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
  onRow: ShowRow;
  onConnect: () => void;
};

/** A TablePlus-style table view: filters, paged canvas grid, structure, footer. */
export function TableTab(props: TableTabProps): React.JSX.Element {
  const { table, sessionId } = props;
  const root = useRef<HTMLElement>(null);
  const state = useTableTab(props);
  const { view, actions, columns } = state;
  useTableKeys(root, actions);
  const editing = useRowEditing({
    ...props,
    target: state.query.target,
    offset: view.offset,
  });
  const showRow = useSelectedRow({
    table: table.name,
    offset: view.offset,
    editing,
    ready: sessionId !== null,
    onRow: props.onRow,
  });
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
