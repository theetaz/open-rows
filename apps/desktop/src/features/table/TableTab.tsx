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
import { TableViewBar } from './TableViewBar';
import type { ShowRow } from './row';
import type { BarAdd, BarData, BarStructure } from './TableViewBar';
import { useDataTools } from './useDataTools';
import type { DataTools } from './useDataTools';
import { useTableKeys } from './useTableKeys';
import { useTableCommandTarget } from './useTableCommandTarget';
import { useTableTab } from './useTableTab';
import { tableTarget } from './model';
import { columnQuickOperators } from './filters/quickFilters';
import type { TableViews } from './useTableViews';
import { RecoveryBanner } from './recovery/RecoveryBanner';
import { useEditRecovery } from './recovery/useEditRecovery';
import { filterLink } from '../links/linkText';
import styles from './Table.module.css';
import { t } from './messages/lazyData';

type TableTabProps = {
  tabId: string;
  table: TableObject;
  profileId: string;
  /** The saved connection's name, for Copy Link. */
  profileName?: string | undefined;
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

/** Reload on schema changes and report unsaved edits to the tab strip. */
function useTabSync(
  props: TableTabProps,
  reload: () => void,
  editing: ReturnType<typeof useRowEditing>,
  structure: ReturnType<typeof useStructure>,
): void {
  useStructureRevision(props.structureRevision, reload);
  useEdited(
    editing.history.current.length + structure.schema.history.current.length,
    props.onEdited,
  );
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
  const { view, columns } = state;
  const ready = sessionId !== null;
  const tools = useDataTools({ ...props, state, editing, ready });
  const tab = { ...state, actions: tools.actions };
  useTableKeys(root, tools.actions);
  const onCommandSelection = useTableCommandTarget({
    root,
    state: tab,
    editing,
    ready,
    tools,
  });
  const structure = useStructure({ ...props, mode: view.mode });
  useTabSync(props, tools.actions.reload, editing, structure);
  const recovery = useEditRecovery({
    tabId: props.tabId,
    target: state.query.target,
    views: props.views,
    editing,
  });
  const context = useTableContext({
    ...state.menu,
    editing,
    jumps: tools.jumps,
    quickOperators: (column) =>
      columnQuickOperators(state.entry.structure, column, props.engine),
  });
  const parts: TabParts = { state: tab, editing, structure, ready, tools };
  return (
    <section
      ref={root}
      className={styles.table}
      aria-label={t('nameTable', { name: table.name })}
    >
      <Banner {...props} />
      <TableViewBar {...barProps(parts)} />
      {view.mode === 'data' && <RecoveryBanner recovery={recovery} />}
      {view.mode === 'data' && view.filtersOpen && (
        <Filters state={tab} engine={props.engine} owner={props} />
      )}
      <DataArea
        tab={tab}
        table={table.name}
        sessionId={sessionId}
        editing={editing}
        tools={tools}
        structure={structure}
        onContextMenu={context.show}
        onSelection={(selection) => {
          onCommandSelection(selection);
          tools.onSelection(selection);
        }}
      />
      {view.columnsOpen && (
        <ColumnsPanel
          columns={columns}
          hidden={view.hidden}
          actions={tools.actions}
        />
      )}
      {context.menu}
      {tools.flow.sheet}
      <EditingFooter {...parts} />
    </section>
  );
}

/** The page's grid or row form in Data mode, else the structure views. */
function DataArea({
  tab,
  table,
  sessionId,
  editing,
  tools,
  structure,
  onContextMenu,
  onSelection,
}: {
  tab: ReturnType<typeof useTableTab>;
  table: string;
  sessionId: string | null;
  editing: ReturnType<typeof useRowEditing>;
  tools: DataTools;
  structure: Structure;
  onContextMenu: React.ComponentProps<typeof TableData>['onContextMenu'];
  onSelection: React.ComponentProps<typeof TableData>['onSelection'];
}): React.JSX.Element | null {
  if (tab.view.mode !== 'data')
    return <StructurePane structure={structure} table={table} />;
  if (!sessionId) return null;
  return (
    <TableData
      state={tab}
      table={table}
      ready
      onRow={tools.onRow}
      editing={editing}
      columns={tab.layout}
      onContextMenu={onContextMenu}
      onSelection={onSelection}
      tools={tools}
    />
  );
}

/** The filter bar of the data view. */
function Filters({
  state,
  engine,
  owner,
}: {
  state: ReturnType<typeof useTableTab>;
  engine: EngineDto;
  /** The tab's connection and relation, for Copy Link. */
  owner: Pick<TableTabProps, 'profileId' | 'profileName' | 'table'>;
}): React.JSX.Element {
  const link = filterLink(
    { id: owner.profileId, name: owner.profileName ?? '' },
    owner.table,
    state.view,
  );
  const { structure } = state.entry;
  const types = new Map(
    structure.status === 'ready'
      ? structure.data.columns.map((column) => [column.name, column.dataType])
      : [],
  );
  return (
    <FilterBar
      view={state.view}
      columns={state.columns}
      types={types}
      engine={engine}
      query={state.query}
      actions={state.actions}
      storageKey={state.storageKey}
      link={link}
    />
  );
}

/** What the view bar and the editing footer read from the tab. */
type TabParts = {
  state: ReturnType<typeof useTableTab>;
  editing: ReturnType<typeof useRowEditing>;
  structure: Structure;
  ready: boolean;
  tools: DataTools;
};

/** The view bar's `+` button and Structure/Index tools for the current mode. */
function barProps({ state, editing, structure, ready, tools }: TabParts): {
  state: TabParts['state'];
  ready: boolean;
  add: BarAdd;
  structure?: BarStructure;
  data?: BarData;
} {
  const { mode } = state.view;
  if (mode === 'data')
    return {
      state,
      ready,
      add: {
        label: t('row'),
        ariaLabel: t('addRow'),
        run: canEdit(editing, ready) ? editing.insert : undefined,
        title: editingNote(editing),
      },
      data: {
        reload: tools.actions.reload,
        toggleForm: tools.actions.toggleForm,
      },
    };
  const { schema } = structure;
  const enabled = schema.enabled;
  const note = schema.detail?.editable === false ? 'Read-only' : '';
  const indexes = mode === 'indexes';
  return {
    state,
    ready,
    add: {
      label: indexes ? t('index') : t('column'),
      ariaLabel: indexes ? t('addIndexLabel') : t('addColumnLabel'),
      run: enabled
        ? indexes
          ? structure.openIndexSheet
          : structure.addColumn
        : undefined,
      title: note || (indexes ? t('addIndex') : t('addColumn')),
    },
    structure: {
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

/** `9 columns` or `3 indexes` for the structure views' status line. */
function structureTotal(
  state: ReturnType<typeof useTableTab>,
  structure: Structure,
): string {
  return state.view.mode === 'indexes'
    ? t('indexTotal', { count: String(structure.indexes.length) })
    : t('columnTotal', { count: String(structure.rows.length) });
}

/** Staged rows, the pending-changes bar, edit errors and the status line. */
function EditingFooter({
  state,
  editing,
  structure,
  ready,
  tools,
}: TabParts): React.JSX.Element {
  const data = state.view.mode === 'data';
  return (
    <>
      {data && (
        <EditToolbar model={rowEditBar(editing, ready, tools.flow.discard)} />
      )}
      {data && editing.error && !editing.review && (
        <p role="alert" className={styles.banner}>
          {editing.error}
        </p>
      )}
      <RowEditSheet editing={editing} />
      <TableFooter
        state={state}
        ready={ready}
        aggregate={tools.aggregate}
        structureStatus={data ? undefined : structureTotal(state, structure)}
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
      {t('notConnected')}{' '}
      <Button size="sm" variant="ghost" onClick={props.onConnect}>
        {t('connectBrowseRows')}
      </Button>
    </p>
  );
}
