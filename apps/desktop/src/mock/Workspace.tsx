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
import { ConnectionSheets, useConnections } from '../features/connections';
import styles from './Workspace.module.css';
/** Real connections and catalog in the rail and sidebar; sample tabs and rows elsewhere. */
export function Workspace({
  onGallery,
  enabled = true,
}: {
  onGallery: () => void;
  enabled?: boolean;
}): React.JSX.Element {
  const connections = useConnections();
  const controller = useWorkspaceController(onGallery, enabled, connections);
  const {
    state,
    catalog,
    compact,
    inspector,
    sidebar,
    safe,
    prod,
    panel,
    setPanel,
    setMode,
    refreshes,
    selection,
    onSelect,
    toggleSidebar,
    addQuery,
    toggleSafe,
    refresh,
  } = controller;
  if (!enabled || !selection.row) return <></>;
  return (
    <div className={styles.workspace}>
      <WorkspaceToolbar
        connections={connections}
        table={state.table}
        schema={catalog.schema || '—'}
        database={catalog.database || 'No database'}
        onDatabase={() => {
          setMode('database');
        }}
        onPalette={() => {
          setMode('commands');
        }}
        onRefresh={refresh}
        safe={safe}
        prod={prod}
        onSafe={toggleSafe}
        sidebar={sidebar}
        inspector={inspector.open}
        onSidebar={toggleSidebar}
        onInspector={inspector.toggle}
        onQuery={addQuery}
      />
      <WorkspaceBody
        connections={connections}
        state={state}
        catalog={catalog}
        onOpen={controller.openObject}
        selection={{ index: selection.index, row: selection.row }}
        panel={panel}
        onPanel={setPanel}
        onSelect={onSelect}
        addQuery={addQuery}
        sidebar={sidebar}
        inspector={inspector.open}
        compact={compact}
        onGallery={onGallery}
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
