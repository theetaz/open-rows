// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { SplitPanel } from '@openrows/ui';
import type { useWorkspaceController } from './useWorkspaceController';
import { inspectorRange, sidebarRange } from './layout';
import {
  ConnectionHeader,
  ConnectionRail,
  connectionActions,
} from '../features/connections';
import type { Connections } from '../features/connections';
import { QueryContent } from '../features/query';
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
          catalog={catalog}
          onOpen={controller.openObject}
          onQuery={() => {
            controller.addQuery();
          }}
        />
      </SplitPanel>
      <QueryContent
        connections={connections}
        tabs={tabs}
        database={scope?.database ?? ''}
        onCell={controller.onCell}
        onRow={controller.onRow}
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
            connectionId={connections.active?.id}
            panel={controller.panel}
            onPanel={controller.setPanel}
            onQuery={controller.addQuery}
          />
        </Suspense>
      </SplitPanel>
    </div>
  );
}

/** Full-height workspace sidebar: connections and their opened databases. */
export function WorkspaceRail(props: {
  connections: Connections;
  onAppearance?: (() => void) | undefined;
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
      onAppearance={props.onAppearance}
    />
  );
}
