// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceBody } from './WorkspaceBody';
const WorkspacePalettes = lazy(() =>
  import('./WorkspacePalettes').then((module) => ({
    default: module.WorkspacePalettes,
  })),
);
import { useWorkspaceController } from './useWorkspaceController';
import { contextSegments } from './contextBar';
import { ConnectionSheets, useConnections } from '../features/connections';
import styles from './Workspace.module.css';
/** Real connections, catalog, query tabs and results. */
export function Workspace({
  onGallery,
  onAppearance,
  enabled = true,
}: {
  onGallery: () => void;
  onAppearance?: (() => void) | undefined;
  enabled?: boolean;
}): React.JSX.Element {
  const connections = useConnections();
  const controller = useWorkspaceController(onGallery, enabled, connections);
  const { catalog, inspector, sidebar, safe, prod, setMode, refreshes } =
    controller;
  if (!enabled) return <></>;
  const tab = controller.scope
    ? controller.tabs.activeFor(controller.scope)
    : undefined;
  const profile = connections.active;
  const status = profile ? connections.statusOf(profile.id) : undefined;
  return (
    <div className={styles.workspace}>
      <WorkspaceToolbar
        context={contextSegments({
          profile,
          status,
          database: catalog.database,
          schema: catalog.schema,
          objectPath: tab?.objectPath,
        })}
        health={status?.state ?? 'disconnected'}
        onDatabase={() => {
          setMode('database');
        }}
        onPalette={() => {
          setMode('commands');
        }}
        onRefresh={controller.refresh}
        safe={safe}
        prod={prod}
        onSafe={controller.toggleSafe}
        sidebar={sidebar}
        inspector={inspector.open}
        onSidebar={controller.toggleSidebar}
        onInspector={inspector.toggle}
        onQuery={() => {
          controller.addQuery();
        }}
      />
      <WorkspaceBody
        connections={connections}
        controller={controller}
        onGallery={onGallery}
        onAppearance={onAppearance}
        onSwitcher={() => {
          setMode('database');
        }}
      />
      <span className={styles.announcement} role="status">
        {refreshes ? `Catalog refreshed (${String(refreshes)})` : ''}
      </span>
      <WorkspaceOverlay controller={controller} />
      <ConnectionSheets connections={connections} />
    </div>
  );
}

function WorkspaceOverlay({
  controller,
}: {
  controller: ReturnType<typeof useWorkspaceController>;
}): React.JSX.Element | null {
  const { mode, setMode, catalog, command } = controller;
  if (!mode) return null;
  return (
    <Suspense fallback={null}>
      <WorkspacePalettes
        key={mode}
        mode={mode}
        catalog={catalog}
        onClose={() => {
          setMode(null);
        }}
        onDatabase={(name) => {
          setMode(null);
          catalog.openDatabase(name);
        }}
        onObject={(object) => {
          setMode(null);
          controller.openObject(object);
        }}
        onCommand={(id) => {
          setMode(null);
          command(id);
        }}
      />
    </Suspense>
  );
}
