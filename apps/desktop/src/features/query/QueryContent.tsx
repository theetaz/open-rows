// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { useTableCloseGuard } from '../table/useTableCloseGuard';
import { Button, Icon } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import { openedDatabase, sessionOf } from '../connections';
import type { Connections } from '../connections';
import type { ProfileDto } from '@openrows/ipc';
import type { CompletionCatalog } from '@openrows/editor';
import { completionCatalog } from './completionCatalog';
import { useTableViews } from '../table';
import type { SelectedTable, ShowRow, TableViews } from '../table';
import type { QueryTabs } from './useQueryTabs';
import { useQueryRuns } from './useQueryRuns';
import type { QueryRuns } from './useQueryRuns';
import type { QueryTab as Tab } from './tabs';
import type { SelectedCell } from './cell';
import { ReconnectBar } from './ReconnectBar';
import { TabStrip } from './TabStrip';
import styles from './Query.module.css';
type SessionStatus = ReturnType<Connections['statusOf']>;
const QueryTab = lazy(() =>
  import('./QueryTab').then((module) => ({ default: module.QueryTab })),
);
const ProcessesTab = lazy(() =>
  import('../processes').then((module) => ({
    default: module.ProcessesTab,
  })),
);
const TableTab = lazy(() =>
  import('../table/TableTab').then((module) => ({
    default: module.TableTab,
  })),
);

type Props = {
  connections: Connections;
  tabs: QueryTabs;
  /** Active database of the connection; empty when none is open. */
  database: string;
  onCell: (cell: SelectedCell) => void;
  /** A table view's selected row, for the inspector's Details. */
  onRow: ShowRow;
  /** Table info for the inspector (Structure/Index modes); `null` clears it. */
  onTableInfo?: ((table: SelectedTable | null) => void) | undefined;
  /** Bumped by schema refresh; drops cached completion data. */
  catalogRevision?: number | undefined;
  /** Open a table of this database (e.g. a foreign key's target). */
  onOpenTable?:
    | ((table: { database: string; schema: string; name: string }) => void)
    | undefined;
  /** Open a trigger editor of a table: new (`name: null`), edit or drop. */
  onTrigger?:
    | ((
        table: { database: string; schema: string; name: string },
        name: string | null,
        drop: boolean,
      ) => void)
    | undefined;
  /** Bumped after an object editor changed the schema; table tabs re-read. */
  structureRevision?: number | undefined;
};

/**
 * Query tabs of the active connection and database; editor and grid load
 * with the first tab.
 */
export function QueryContent(props: Props): React.JSX.Element {
  const runs = useQueryRuns();
  const tables = useTableViews();
  const closeGuard = useTableCloseGuard({ tables, runs, tabs: props.tabs });
  const connection = props.connections.active;
  if (!connection)
    return (
      <main className={styles.content}>
        <EmptyState
          icon="database"
          title="No connection selected"
          detail="Choose a connection in the sidebar to open a query tab."
          action="New connection…"
          onAction={() => {
            props.connections.setSheet({ kind: 'new' });
          }}
        />
      </main>
    );
  if (!props.database)
    return (
      <main className={styles.content}>
        <EmptyState
          icon="database"
          title="No database open"
          detail="Open a database with ⌘K to start a query tab."
        />
      </main>
    );
  const { tabs } = props;
  const scope = { connectionId: connection.id, database: props.database };
  const list = tabs.tabsFor(scope);
  const tab = tabs.activeFor(scope);
  const status = props.connections.statusOf(connection.id);
  const open = (): void => {
    tabs.open(scope);
  };
  return (
    <main className={styles.content}>
      {closeGuard}
      {list.length > 0 && (
        <ReconnectBar
          profile={connection}
          status={status}
          onConnect={props.connections.connect}
          database={openedDatabase(status, props.database)}
          onReopen={() => {
            props.connections.openDatabase(props.database);
          }}
        />
      )}
      <TabStrip
        tabs={tabs}
        list={list}
        active={tab?.id ?? ''}
        connection={connection.name}
        onAdd={open}
      />
      {tab ? (
        <Suspense fallback={<p className={styles.empty}>Loading…</p>}>
          <TabBody
            {...props}
            tab={tab}
            connection={connection}
            status={status}
            runs={runs}
            tables={tables}
          />
        </Suspense>
      ) : (
        <NoTabs database={props.database} onOpen={open} />
      )}
    </main>
  );
}

/** The active tab: a process list, a table view or a query editor. */
function TabBody(
  props: Props & {
    tab: Tab;
    connection: ProfileDto;
    status: SessionStatus;
    runs: QueryRuns;
    tables: TableViews;
  },
): React.JSX.Element {
  const { tab, status } = props;
  const sessionId = sessionOf(status, tab.database);
  const reason = notice(status, tab.database);
  if (tab.kind === 'processes')
    return (
      <ProcessesTab
        key={tab.id}
        profile={props.connection}
        database={tab.database}
        connected={status.state === 'connected'}
        onRow={props.onRow}
      />
    );
  if (tab.kind === 'table' && tab.table)
    return (
      <TableTab
        key={tab.id}
        tabId={tab.id}
        table={tab.table}
        profileId={props.connection.id}
        engine={props.connection.engine}
        features={props.connection.objects}
        sessionId={sessionId}
        notice={reason}
        views={props.tables}
        onRow={props.onRow}
        onConnect={props.connections.connect}
        onTableInfo={props.onTableInfo}
        onOpenTable={(schema, name) => {
          props.onOpenTable?.({ database: tab.database, schema, name });
        }}
        onRenamed={(name) => {
          props.tabs.renameTable?.(tab.id, name);
        }}
        onTrigger={(name, drop) => {
          if (tab.table) props.onTrigger?.(tab.table, name, drop);
        }}
        structureRevision={props.structureRevision}
        onEdited={() => {
          props.tabs.keep(tab.id);
        }}
      />
    );
  return (
    <QueryTab
      key={tab.id}
      tab={tab}
      connection={props.connection}
      sessionId={sessionId}
      notice={reason}
      catalog={completionFor(props, sessionId)}
      runs={props.runs}
      tabs={props.tabs}
      onCell={props.onCell}
      onConnect={props.connections.connect}
    />
  );
}

function NoTabs(props: {
  database: string;
  onOpen: () => void;
}): React.JSX.Element {
  return (
    <EmptyState
      icon="terminal"
      title={`No query tabs on ${props.database}.`}
      detail="Open a table from the sidebar, or start a new query."
      action="New query tab (⌘T)"
      onAction={props.onOpen}
    />
  );
}

/** Centred icon, title, hint and optional action for an empty content area. */
function EmptyState(props: {
  icon: IconName;
  title: string;
  detail: string;
  action?: string;
  onAction?: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <span className={styles.emptyIcon} aria-hidden="true">
        <Icon name={props.icon} />
      </span>
      <p className={styles.emptyTitle}>{props.title}</p>
      <p>{props.detail}</p>
      {props.action && (
        <Button size="sm" onClick={props.onAction}>
          {props.action}
        </Button>
      )}
    </div>
  );
}

/** The tab database's completion catalog; none without a session. */
function completionFor(
  props: Props & { tab: Tab; connection: ProfileDto; status: SessionStatus },
  sessionId: string | null,
): CompletionCatalog | null {
  if (!sessionId || props.status.state !== 'connected') return null;
  return completionCatalog({
    profileId: props.connection.id,
    database: props.tab.database,
    revision: `${String(props.status.generation)}:${String(props.catalogRevision ?? 0)}`,
  });
}

/** Why a tab cannot run although its connection is up. */
function notice(status: SessionStatus, database: string): string | null {
  if (status.state !== 'connected') return null;
  const opened = status.databases.find((item) => item.name === database);
  if (!opened) return `${database} is closed; open it again with ⌘K`;
  if (opened.state === 'opening') return `Opening ${database}…`;
  return opened.state === 'failed' ? opened.message : null;
}
