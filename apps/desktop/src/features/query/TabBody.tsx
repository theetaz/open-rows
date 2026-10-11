// SPDX-License-Identifier: Apache-2.0
import { lazy } from 'react';
import type { ProfileDto } from '@openrows/ipc';
import type { CompletionCatalog } from '@openrows/editor';
import { sessionOf } from '../connections';
import type { Connections } from '../connections';
import type { TableViews } from '../table';
import type { QueryContentProps } from './QueryContent';
import type { QueryTabs } from './useQueryTabs';
import type { QueryRuns } from './useQueryRuns';
import type { QueryTab as Tab } from './tabs';
import { completionCatalog } from './completionCatalog';
type SessionStatus = ReturnType<Connections['statusOf']>;
const QueryTab = lazy(() =>
  import('./QueryTab').then((module) => ({ default: module.QueryTab })),
);
const ProcessesTab = lazy(() =>
  import('../processes').then((module) => ({ default: module.ProcessesTab })),
);
const TableTab = lazy(() =>
  import('../table/TableTab').then((module) => ({ default: module.TableTab })),
);
const InspectionTab = lazy(() =>
  import('../inspection/InspectionTab').then((module) => ({
    default: module.InspectionTab,
  })),
);

/** The active tab: a process list, a table view or a query editor. */
export function TabBody(
  props: QueryContentProps & {
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
  if (tab.kind === 'inspection' && tab.inspection)
    return (
      <CatalogTab
        tab={tab}
        tabs={props.tabs}
        connected={status.state === 'connected' && !!sessionId}
        revision={
          (props.catalogRevision ?? 0) +
          (status.state === 'connected' ? status.generation : 0)
        }
      />
    );
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

/** An inspection tab opens its DDL in the same database's query editor. */
function CatalogTab(props: {
  tab: Tab;
  tabs: QueryTabs;
  connected: boolean;
  revision: number;
}): React.JSX.Element | null {
  const { tab } = props;
  if (!tab.inspection) return null;
  return (
    <InspectionTab
      profileId={tab.connectionId}
      database={tab.database}
      target={tab.inspection}
      connected={props.connected}
      revision={props.revision}
      onOpenSql={(sql) => {
        props.tabs.open(
          { connectionId: tab.connectionId, database: tab.database },
          { sql, title: `${tab.title} DDL` },
        );
      }}
    />
  );
}

/** The tab database's completion catalog; none without a session. */
function completionFor(
  props: QueryContentProps & {
    tab: Tab;
    connection: ProfileDto;
    status: SessionStatus;
  },
  sessionId: string | null,
): CompletionCatalog | null {
  if (
    !sessionId ||
    props.status.state !== 'connected' ||
    props.connection.engine === 'duckdb'
  )
    return null;
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
