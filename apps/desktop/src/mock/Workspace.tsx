// SPDX-License-Identifier: Apache-2.0
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceBody } from './WorkspaceBody';
import { WorkspacePalettes } from './WorkspacePalettes';
import { useWorkspaceController } from './useWorkspaceController';
import styles from './Workspace.module.css';
/** Hard-coded connection workspace; all commands mutate local preview state only. */
export function Workspace({
  onGallery,
  enabled = true,
}: {
  onGallery: () => void;
  enabled?: boolean;
}): React.JSX.Element {
  const controller = useWorkspaceController(onGallery, enabled);
  const {
    state,
    compact,
    inspector,
    sidebar,
    safe,
    panel,
    setPanel,
    setMode,
    refreshes,
    selection,
    onSelect,
    toggleSidebar,
    addQuery,
    connection,
    toggleSafe,
    refresh,
  } = controller;
  if (!connection || !selection.row) return <></>;
  return (
    <div className={styles.workspace}>
      <WorkspaceToolbar
        connection={connection}
        table={state.table}
        schema={state.schema}
        database={state.database}
        onDatabase={() => {
          setMode('database');
        }}
        onPalette={() => {
          setMode('commands');
        }}
        onRefresh={refresh}
        safe={safe || connection.environment === 'prod'}
        onSafe={toggleSafe}
        sidebar={sidebar}
        inspector={inspector.open}
        onSidebar={toggleSidebar}
        onInspector={inspector.toggle}
        onQuery={addQuery}
      />
      <WorkspaceBody
        state={state}
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
        {refreshes ? `Mock catalog refreshed (${String(refreshes)})` : ''}
      </span>
      <WorkspaceOverlay controller={controller} />
    </div>
  );
}

function WorkspaceOverlay({
  controller,
}: {
  controller: ReturnType<typeof useWorkspaceController>;
}): React.JSX.Element | null {
  const { mode, setMode, state, command } = controller;
  if (!mode) return null;
  return (
    <WorkspacePalettes
      key={mode}
      mode={mode}
      connection={state.connection}
      catalog={state.catalog}
      opened={state.opened}
      onClose={() => {
        setMode(null);
      }}
      onDatabase={(name) => {
        setMode(null);
        state.openDatabase(name);
      }}
      onObject={(object) => {
        setMode(null);
        state.openTable(object);
      }}
      onCommand={(id) => {
        setMode(null);
        command(id);
      }}
    />
  );
}
