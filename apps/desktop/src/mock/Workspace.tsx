// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { SplitPanel } from '@openrows/ui';
import { connections } from './data/connections';
import { rows } from './data/rows';
import type { Connection } from './data/connections';
import type { OrderRow } from './data/rows';
import { useWorkspace } from './useWorkspace';
import {
  inspectorRange,
  sidebarRange,
  useCompact,
  useInspectorOpen,
} from './layout';
import { shortcut } from './shortcuts';
import { ConnectionRail } from './ConnectionRail';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { WorkspaceInspector } from './WorkspaceInspector';
import { TableContent } from './TableContent';
import styles from './Workspace.module.css';

/** Approved workspace rebuilt as isolated, hard-coded UI state. */
export function Workspace({
  onGallery,
  enabled = true,
}: {
  onGallery: () => void;
  enabled?: boolean;
}): React.JSX.Element {
  const state = useWorkspace();
  const compact = useCompact();
  const inspector = useInspectorOpen(compact);
  const [sidebar, setSidebar] = useState(true);
  const [safe, setSafe] = useState(false);
  const [panel, setPanel] = useState('Assistant');
  const [selection, setSelection] = useState({ index: 3, row: rows[3] });
  const onSelect = useCallback((index: number, row: OrderRow) => {
    setSelection({ index, row });
    setPanel('Details');
  }, []);
  const toggleSidebar = (): void => {
    setSidebar((open) => !open);
  };
  const { addTab } = state;
  const addQuery = useCallback(() => {
    addTab();
  }, [addTab]);
  useWorkspaceKeys({
    enabled,
    sidebar: toggleSidebar,
    inspector: inspector.toggle,
    add: addQuery,
    close: () => {
      state.closeTab(state.active);
    },
    connection: state.selectConnection,
  });
  const connection = connections[state.connection];
  if (!connection || !selection.row) return <></>;
  return (
    <div className={styles.workspace}>
      <WorkspaceToolbar
        connection={connection}
        table={state.active}
        schema={state.schema}
        safe={safe || connection.environment === 'prod'}
        onSafe={() => {
          if (connection.environment !== 'prod') setSafe(!safe);
        }}
        sidebar={sidebar}
        inspector={inspector.open}
        onSidebar={toggleSidebar}
        onInspector={inspector.toggle}
        onQuery={addQuery}
      />
      <WorkspaceBody
        state={state}
        connection={connection}
        selection={{ index: selection.index, row: selection.row }}
        panel={panel}
        onPanel={setPanel}
        onSelect={onSelect}
        addQuery={addQuery}
        sidebar={sidebar}
        inspector={inspector.open}
        compact={compact}
        onGallery={onGallery}
      />
    </div>
  );
}
function WorkspaceBody({
  state,
  connection,
  selection,
  panel,
  onPanel,
  onSelect,
  addQuery,
  sidebar,
  inspector,
  compact,
  onGallery,
}: {
  state: ReturnType<typeof useWorkspace>;
  connection: Connection;
  selection: { index: number; row: OrderRow };
  panel: string;
  onPanel: (panel: string) => void;
  onSelect: (index: number, row: OrderRow) => void;
  addQuery: () => void;
  sidebar: boolean;
  inspector: boolean;
  compact: boolean;
  onGallery: () => void;
}): React.JSX.Element {
  return (
    <div className={styles.body}>
      <ConnectionRail
        selected={state.connection}
        onSelect={state.selectConnection}
        onGallery={onGallery}
      />
      <SplitPanel
        label="Sidebar"
        storageKey="openrows.mock.sidebar"
        range={sidebarRange}
        open={sidebar}
        edge="right"
      >
        <WorkspaceSidebar
          connection={connection}
          table={state.active}
          schema={state.schema}
          onSchema={state.setSchema}
          onTable={state.openTable}
          onQuery={addQuery}
        />
      </SplitPanel>
      <TableContent
        tabs={state.tabs}
        active={state.active}
        selected={selection.index}
        onSelect={onSelect}
        onTab={state.selectTab}
        onClose={state.closeTab}
        onAdd={addQuery}
      />
      <SplitPanel
        label="Inspector"
        storageKey="openrows.mock.inspector"
        range={inspectorRange}
        open={inspector}
        edge="left"
        overlay={compact}
      >
        <WorkspaceInspector
          row={selection.row}
          index={selection.index}
          panel={panel}
          onPanel={onPanel}
          onQuery={state.addTab}
          table={state.active}
          schema={state.schema}
        />
      </SplitPanel>
    </div>
  );
}
function useWorkspaceKeys(actions: {
  enabled: boolean;
  sidebar: () => void;
  inspector: () => void;
  add: () => void;
  close: () => void;
  connection: (index: number) => void;
}): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (!actions.enabled || document.querySelector('dialog[open]')) return;
      const command = shortcut(event);
      if (command === undefined) return;
      event.preventDefault();
      if (typeof command === 'number') actions.connection(command);
      else
        ({
          sidebar: actions.sidebar,
          inspector: actions.inspector,
          'new-tab': actions.add,
          'close-tab': actions.close,
        })[command]();
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, [actions]);
}
