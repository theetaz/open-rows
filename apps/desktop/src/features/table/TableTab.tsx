// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { Button } from '@openrows/ui';
import type { TableObject } from '../query/tabs';
import { canEdit, EditToolbar, editingNote, rowEditBar } from './EditToolbar';
import { RowEditSheet } from './RowEditSheet';
import { useRowEditing } from './useRowEditing';
import { useTableContext } from './TableContextMenu';
import { ColumnsPanel } from './ColumnsPanel';
import { FilterBar } from './FilterBar';
import { StructurePane } from './structure/StructurePane';
import { useStructure } from './structure/useStructure';
import type { Structure } from './structure/useStructure';
import type { SelectedTable } from './structure/TableInfoPanel';
import { TableData } from './TableData';
import { TableFooter } from './TableFooter';
import type { ShowRow } from './row';
import { useSelectedRow } from './useSelectedRow';
import type { FooterAdd, FooterStructure } from './TableFooter';
import { useTableKeys } from './useTableKeys';
import { useTableTab } from './useTableTab';
import type { TableViews } from './useTableViews';
import { RecoveryBanner } from './recovery/RecoveryBanner';
import { useEditRecovery } from './recovery/useEditRecovery';
import styles from './Table.module.css';

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
  /** Table info for the inspector in Structure/Index mode; `null` hides it. */
  onTableInfo?: ((table: SelectedTable | null) => void) | undefined;
  /** Open another table (a foreign key's target) in a tab. */
  onOpenTable?: ((schema: string, table: string) => void) | undefined;
  /** The table was renamed by a committed structure change. */
  onRenamed?: ((name: string) => void) | undefined;
};

/** A TablePlus-style table view: filters, paged canvas grid, structure, footer. */
export function TableTab(props: TableTabProps): React.JSX.Element {
  const { table, sessionId } = props;
  const root = useRef<HTMLElement>(null);
  const state = useTableTab(props);
  const { view, actions, columns } = state;
  useTableKeys(root, actions);
  const structure = useStructure({ ...props, mode: view.mode });
  const editing = useRowEditing({
    ...props,
    target: state.query.target,
    offset: view.offset,
  });
  const recovery = useEditRecovery({
    tabId: props.tabId,
    target: state.query.target,
    views: props.views,
    editing,
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
      {view.mode === 'data' && <RecoveryBanner recovery={recovery} />}
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
            editing={editing}
            onContextMenu={context.show}
          />
        )
      ) : (
        <StructurePane structure={structure} table={table.name} />
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
        structure={structure}
        ready={sessionId !== null}
      />
    </section>
  );
}

/** The footer's `+` button and Structure/Index tools for the current mode. */
function footerProps(
  state: ReturnType<typeof useTableTab>,
  editing: ReturnType<typeof useRowEditing>,
  structure: Structure,
  ready: boolean,
): { add: FooterAdd; structure?: FooterStructure } {
  const { mode } = state.view;
  if (mode === 'data')
    return {
      add: {
        label: 'Row',
        run: canEdit(editing, ready) ? editing.insert : undefined,
        title: editingNote(editing),
      },
    };
  const { schema } = structure;
  const enabled = schema.enabled;
  const note = schema.detail?.editable === false ? 'Read-only' : '';
  const indexes = mode === 'indexes';
  return {
    add: {
      label: indexes ? 'Index' : 'Column',
      run: enabled
        ? indexes
          ? structure.openIndexSheet
          : structure.addColumn
        : undefined,
      title: note || (indexes ? 'Add an index' : 'Add a column'),
    },
    structure: {
      status: indexes
        ? `${String(structure.indexes.length)} indexes`
        : `${String(structure.rows.length)} columns`,
      onTriggers: () => {
        structure.setSheet('triggers');
      },
      onDdl: structure.toggleDdl,
      ddlOpen: structure.ddl !== null,
    },
  };
}

/** Staged rows, the pending-changes bar, edit errors and the table footer. */
function EditingFooter({
  state,
  editing,
  structure,
  ready,
}: {
  state: ReturnType<typeof useTableTab>;
  editing: ReturnType<typeof useRowEditing>;
  structure: Structure;
  ready: boolean;
}): React.JSX.Element {
  const data = state.view.mode === 'data';
  return (
    <>
      {data && <EditToolbar model={rowEditBar(editing, ready)} />}
      {data && editing.error && !editing.review && (
        <p role="alert" className={styles.banner}>
          {editing.error}
        </p>
      )}
      <RowEditSheet editing={editing} />
      <TableFooter
        state={state}
        ready={ready}
        {...footerProps(state, editing, structure, ready)}
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
