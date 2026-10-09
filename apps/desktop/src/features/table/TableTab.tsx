// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { Button } from '@openrows/ui';
import type { EngineDto, ObjectFeaturesDto } from '@openrows/ipc';
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
import type { TriggerAction } from './structure/TriggersSheet';
import { TableData } from './TableData';
import { TableFooter } from './TableFooter';
import type { ShowRow } from './row';
import { useSelectedRow } from './useSelectedRow';
import type { FooterAdd, FooterStructure } from './TableFooter';
import { useTableKeys } from './useTableKeys';
import { useTableCommandTarget } from './useTableCommandTarget';
import { useTableTab } from './useTableTab';
import { tableTarget } from './model';
import type { TableViews } from './useTableViews';
import { RecoveryBanner } from './recovery/RecoveryBanner';
import { useEditRecovery } from './recovery/useEditRecovery';
import styles from './Table.module.css';

type TableTabProps = {
  tabId: string;
  table: TableObject;
  profileId: string;
  /** Engine of the connection: quoting for copied SQL, types and forms. */
  engine: EngineDto;
  /** The structure editors the engine supports. */
  features: ObjectFeaturesDto;
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
  /** Something was staged: a preview tab becomes a normal tab. */
  onEdited?: (() => void) | undefined;
  /** Open the trigger editors of this table (new, edit or drop). */
  onTrigger?: TriggerAction | undefined;
  /** Bumped after an object editor changed the schema: re-read the table. */
  structureRevision?: number | undefined;
};

/** Read the table again when an object editor changed the schema. */
function useStructureRevision(
  revision: number | undefined,
  reload: () => void,
): void {
  const seen = useRef(revision);
  const latest = useRef(reload);
  useEffect(() => {
    latest.current = reload;
  });
  useEffect(() => {
    if (seen.current === revision) return;
    seen.current = revision;
    latest.current();
  }, [revision]);
}

/** Report the first staged row or schema change. */
function useEdited(staged: number, onEdited: (() => void) | undefined): void {
  const edited = staged > 0;
  useEffect(() => {
    if (edited) onEdited?.();
  }, [edited, onEdited]);
}

/** A TablePlus-style table view: filters, paged canvas grid, structure, footer. */
export function TableTab(props: TableTabProps): React.JSX.Element {
  const { table, sessionId } = props;
  const root = useRef<HTMLElement>(null);
  const editing = useRowEditing({
    ...props,
    target: tableTarget(table, props.profileId),
    offset: props.views.entryOf(props.tabId).view.offset,
  });
  const state = useTableTab({ ...props, rowId: editing.info?.rowId ?? null });
  const { view, actions, columns } = state;
  useTableKeys(root, actions);
  const onSelection = useTableCommandTarget({
    root,
    state,
    editing,
    ready: sessionId !== null,
  });
  const structure = useStructure({ ...props, mode: view.mode });
  useStructureRevision(props.structureRevision, actions.reload);
  useEdited(
    editing.history.current.length + structure.schema.history.current.length,
    props.onEdited,
  );
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
  const context = useTableContext({ ...state.menu, editing });
  return (
    <section
      ref={root}
      className={styles.table}
      aria-label={`${table.name} table`}
    >
      <Banner {...props} />
      {view.mode === 'data' && <RecoveryBanner recovery={recovery} />}
      {view.mode === 'data' && view.filtersOpen && <Filters state={state} />}
      {view.mode === 'data' ? (
        sessionId && (
          <TableData
            state={state}
            onRow={showRow}
            editing={editing}
            columns={state.layout}
            onContextMenu={context.show}
            onSelection={onSelection}
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

/** The filter bar of the data view. */
function Filters({
  state,
}: {
  state: ReturnType<typeof useTableTab>;
}): React.JSX.Element {
  return (
    <FilterBar
      view={state.view}
      columns={state.columns}
      query={state.query}
      actions={state.actions}
    />
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
      onConstraints: () => {
        structure.setSheet('constraints');
      },
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
