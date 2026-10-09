// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceBody, WorkspaceRail } from './WorkspaceBody';
const WorkspacePalettes = lazy(() =>
  import('./WorkspacePalettes').then((module) => ({
    default: module.WorkspacePalettes,
  })),
);
import { useWorkspaceController } from './useWorkspaceController';
import type { DiagramRequest } from './useWorkspaceController';
import { contextSegments } from './contextBar';
import { ConnectionSheets, SshPrompts, useConnections } from '../connections';
import type { SettingsSection } from '../settings';
import { TableExport } from './TableExport';
import { TableImport } from './TableImport';
import type { ImportTarget } from './TableImport';
import { ErDiagram } from '../erd';
import styles from './Workspace.module.css';
/** Real connections, catalog, query tabs and results. */
export function Workspace({
  onGallery,
  onSettings = () => undefined,
  enabled = true,
}: {
  onGallery: () => void;
  onSettings?: (section?: SettingsSection) => void;
  enabled?: boolean;
}): React.JSX.Element {
  const connections = useConnections();
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState<ImportTarget | null>(null);
  const [diagram, setDiagram] = useState<DiagramRequest | null>(null);
  const controller = useWorkspaceController({
    onGallery,
    onSettings,
    onExportTable: () => {
      setExporting(true);
    },
    onImport: setImporting,
    onShowDiagram: setDiagram,
    enabled,
    connections,
  });
  if (!enabled) return <></>;
  return (
    <div className={styles.workspace}>
      <WorkspaceRail
        connections={connections}
        onSettings={() => {
          onSettings();
        }}
        onProcesses={controller.openProcesses}
      />
      <div className={styles.main}>
        <Toolbar connections={connections} controller={controller} />
        <WorkspaceBody connections={connections} controller={controller} />
      </div>
      <span className={styles.announcement} role="status">
        {controller.refreshes
          ? `Catalog refreshed (${String(controller.refreshes)})`
          : ''}
      </span>
      <WorkspaceOverlay controller={controller} />
      <TransferSheets
        connections={connections}
        controller={controller}
        exporting={exporting}
        importing={importing}
        onClose={() => {
          setExporting(false);
          setImporting(null);
        }}
      />
      {diagram && connections.active && (
        <ErDiagram
          key={`${diagram.database}\n${diagram.schema}`}
          {...diagram}
          profileId={connections.active.id}
          onOpen={(name) => {
            controller.openObject({
              database: diagram.database,
              schema: diagram.schema,
              name,
              kind: 'table',
              arguments: null,
            });
          }}
          onClose={() => {
            setDiagram(null);
          }}
        />
      )}
      <ConnectionSheets connections={connections} />
      <SshPrompts />
    </div>
  );
}

/** File ▸ Export Table… and Import… sheets. */
function TransferSheets(props: {
  connections: ReturnType<typeof useConnections>;
  controller: ReturnType<typeof useWorkspaceController>;
  exporting: boolean;
  importing: ImportTarget | null;
  onClose: () => void;
}): React.JSX.Element {
  const { connections, controller } = props;
  return (
    <>
      {props.exporting && (
        <TableExport
          connections={connections}
          tabs={controller.tabs}
          scope={controller.scope}
          onClose={props.onClose}
        />
      )}
      {props.importing && (
        <TableImport
          connections={connections}
          target={props.importing}
          onClose={props.onClose}
        />
      )}
    </>
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
        hidden={controller.hiddenCommands}
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

/** The titlebar toolbar: context bar, panels, safe mode and actions. */
function Toolbar({
  connections,
  controller,
}: {
  connections: ReturnType<typeof useConnections>;
  controller: ReturnType<typeof useWorkspaceController>;
}): React.JSX.Element {
  const { catalog, inspector, sidebar, safe, prod, setMode } = controller;
  const tab = controller.scope
    ? controller.tabs.activeFor(controller.scope)
    : undefined;
  const profile = connections.active;
  const status = profile ? connections.statusOf(profile.id) : undefined;
  return (
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
  );
}
