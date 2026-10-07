// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { SplitPanel } from '@openrows/ui';
import type { useWorkspace } from './useWorkspace';
import type { OrderRow } from './data/rows';
import { inspectorRange, sidebarRange } from './layout';
import { ConnectionHeader, ConnectionRail } from '../features/connections';
import type { Connections } from '../features/connections';
import { WorkspaceSidebar } from './WorkspaceSidebar';
const WorkspaceInspector = lazy(() =>
  import('./WorkspaceInspector').then((module) => ({
    default: module.WorkspaceInspector,
  })),
);
import { TableContent } from './TableContent';
import styles from './Workspace.module.css';
type Props = {
  connections: Connections;
  state: ReturnType<typeof useWorkspace>;
  selection: { index: number; row: OrderRow };
  panel: string;
  onPanel: (panel: string) => void;
  onSelect: (index: number, row: OrderRow) => void;
  addQuery: () => void;
  sidebar: boolean;
  inspector: boolean;
  compact: boolean;
  onGallery: () => void;
  onSwitcher: () => void;
};
/** Database-specific documents and object browser inside the preview shell. */
export function WorkspaceBody(props: Props): React.JSX.Element {
  const {
    connections,
    state,
    selection,
    panel,
    onPanel,
    onSelect,
    addQuery,
    sidebar,
    inspector,
    compact,
    onGallery,
    onSwitcher,
  } = props;
  return (
    <div className={styles.body}>
      <Rail connections={connections} onGallery={onGallery} />
      <SplitPanel
        label="Sidebar"
        storageKey="openrows.mock.sidebar"
        range={sidebarRange}
        open={sidebar}
        edge="right"
      >
        <WorkspaceSidebar
          header={<ConnectionHeader connections={connections} />}
          opened={state.opened}
          onDatabase={state.openDatabase}
          onCloseDatabase={state.closeDatabase}
          onSwitcher={onSwitcher}
          database={state.catalog}
          table={state.table}
          schema={state.schema}
          onSchema={state.setSchema}
          onTable={state.openTable}
          onQuery={addQuery}
        />
      </SplitPanel>
      <TableContent
        {...{ database: state.database, tabs: state.tabs }}
        active={state.active}
        selected={selection.index}
        onSelect={onSelect}
        onTab={state.selectTab}
        onClose={state.closeTab}
        onAdd={addQuery}
        onSql={state.editSql}
        onTable={state.editTable}
      />
      <SplitPanel
        label="Inspector"
        storageKey="openrows.mock.inspector"
        range={inspectorRange}
        open={inspector}
        edge="left"
        overlay={compact}
      >
        <Suspense fallback={null}>
          <WorkspaceInspector
            row={selection.row}
            index={selection.index}
            panel={panel}
            onPanel={onPanel}
            onQuery={state.addTab}
            table={state.table}
            schema={state.schema}
          />
        </Suspense>
      </SplitPanel>
    </div>
  );
}

function Rail(props: {
  connections: Connections;
  onGallery: () => void;
}): React.JSX.Element {
  const { connections } = props;
  return (
    <ConnectionRail
      profiles={connections.profiles}
      activeId={connections.active?.id}
      statusOf={connections.statusOf}
      onSelect={connections.select}
      onNew={() => {
        connections.setSheet({ kind: 'new' });
      }}
      onGallery={props.onGallery}
    />
  );
}
