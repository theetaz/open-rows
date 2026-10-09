// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { Icon, IconButton, SplitPanel } from '@openrows/ui';
import type { useWorkspaceController } from './useWorkspaceController';
import { inspectorRange, sidebarRange } from './layout';
import {
  ConnectionHeader,
  ConnectionRail,
  connectionActions,
  sessionOf,
} from '../connections';
import type { Connections } from '../connections';
import { QueryContent } from '../query';
import { AttachDatabaseTools } from '../schema';
import { WorkspaceSidebar } from './WorkspaceSidebar';
const WorkspaceInspector = lazy(() =>
  import('./WorkspaceInspector').then((module) => ({
    default: module.WorkspaceInspector,
  })),
);
import styles from './Workspace.module.css';
type Props = {
  connections: Connections;
  controller: ReturnType<typeof useWorkspaceController>;
};
/** Object browser, query tabs and the inspector. */
export function WorkspaceBody(props: Props): React.JSX.Element {
  const { connections, controller } = props;
  const { catalog, tabs, scope } = controller;
  return (
    <div className={styles.body}>
      <SplitPanel
        label="Sidebar"
        storageKey="openrows.mock.sidebar"
        range={sidebarRange}
        open={controller.sidebar}
        edge="right"
      >
        <WorkspaceSidebar
          header={<ConnectionHeader connections={connections} />}
          schemaTools={<SchemaTools {...props} />}
          catalog={catalog}
          connections={connections}
          onOpen={controller.openObject}
          onDiagram={controller.showDiagram}
          onOpenSql={(sql) => {
            controller.addQuery(sql);
          }}
          onSavedQuery={(query) => {
            if (scope) tabs.open(scope, { title: query.name, sql: query.sql });
          }}
          sessionFor={(database) => {
            const profile = connections.active;
            const sessionId = profile
              ? sessionOf(connections.statusOf(profile.id), database)
              : null;
            return profile && sessionId
              ? { profileId: profile.id, sessionId }
              : null;
          }}
        />
      </SplitPanel>
      <QueryContent
        connections={connections}
        tabs={tabs}
        database={scope?.database ?? ''}
        onCell={controller.onCell}
        onRow={controller.onRow}
        onTableInfo={controller.onTable}
        catalogRevision={controller.catalog.revision}
        onOpenTable={(table) => {
          controller.openObject({ ...table, kind: 'table', arguments: null });
        }}
      />
      <SplitPanel
        label="Inspector"
        storageKey="openrows.mock.inspector"
        range={inspectorRange}
        open={controller.inspector.open}
        edge="left"
        overlay={controller.compact}
      >
        <Suspense fallback={null}>
          <WorkspaceInspector
            cell={controller.cell}
            row={controller.row}
            table={controller.table}
          />
        </Suspense>
      </SplitPanel>
    </div>
  );
}

/** Tools beside the schema picker: the ER diagram and engine extras. */
function SchemaTools({ connections, controller }: Props): React.JSX.Element {
  const { catalog } = controller;
  return (
    <>
      <IconButton
        aria-label="Show ER diagram"
        title="Show ER Diagram"
        size="sm"
        variant="ghost"
        disabled={!catalog.schema}
        onClick={() => {
          controller.showDiagram();
        }}
      >
        <Icon name="diagram" />
      </IconButton>
      <AttachDatabaseTools connections={connections} catalog={catalog} />
    </>
  );
}

/** Full-height workspace sidebar: connections and their opened databases. */
export function WorkspaceRail(props: {
  connections: Connections;
  onSettings?: (() => void) | undefined;
}): React.JSX.Element {
  const { connections } = props;
  return (
    <ConnectionRail
      profiles={connections.profiles}
      activeId={connections.active?.id}
      statusOf={connections.statusOf}
      onSelect={connections.select}
      databases={connections.databases}
      database={connections.database}
      onDatabase={connections.openDatabase}
      onCloseDatabase={connections.closeDatabase}
      onProfileDatabase={(id, name) => {
        connections.select(id);
        connections.chooseDatabase(id, name);
      }}
      onCloseProfileDatabase={connections.closeProfileDatabase}
      onNew={() => {
        connections.setSheet({ kind: 'new' });
      }}
      actionsFor={(profile) => connectionActions(profile, connections)}
      onSettings={props.onSettings}
    />
  );
}
