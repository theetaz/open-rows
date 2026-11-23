// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { WorkspaceUpdates } from './WorkspaceUpdates';
import { TabStripHost } from '../query/TabStrip';
import { WorkspaceTop } from './WorkspaceTop';
import { FileNotice, RefreshAnnouncement } from './WorkspaceNotices';
import { WorkspaceBody } from './WorkspaceBody';
import { SidebarColumn } from './SidebarColumn';
const WorkspacePalettes = lazy(() =>
  import('./WorkspacePalettes').then((module) => ({
    default: module.WorkspacePalettes,
  })),
);
import { openQueryFile } from '../query';
import { reportProjectError } from '../projects';
import { useWorkspaceController } from './useWorkspaceController';
import type { DiagramRequest } from './useWorkspaceController';
import { ConnectionSheets, SshPrompts, useConnections } from '../connections';
import type { SettingsSection } from '../settings';
import { TableExport } from './TableExport';
import { TableImport } from './TableImport';
import type { ImportTarget } from './TableImport';
import { JobSheets, useJobActions } from './JobActions';
import type { JobActions } from './JobActions';
import { ErDiagram } from '../erd';
import { ObjectEditors } from '../objects';
import styles from './Frame.module.css';
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
  const jobs = useJobActions(connections);
  const controller = useWorkspaceController({
    onGallery,
    onSettings,
    onExportTable: () => {
      setExporting(true);
    },
    onImport: setImporting,
    onShowDiagram: setDiagram,
    jobs,
    enabled,
    connections,
  });
  if (!enabled) return <></>;
  return (
    <div className={styles.workspace}>
      <SidebarColumn connections={connections} controller={controller} />
      <div className={styles.main}>
        <TabStripHost value={controller.windowTabs.host}>
          <WorkspaceTop connections={connections} controller={controller} />
          <WorkspaceUpdates connections={connections} tabs={controller.tabs} />
          <FileNotice notice={controller.fileNotice} />
          <WorkspaceBody connections={connections} controller={controller} />
        </TabStripHost>
      </div>
      <RefreshAnnouncement refreshes={controller.refreshes} />
      <WorkspaceOverlay controller={controller} connections={connections} />
      <TransferSheets
        connections={connections}
        controller={controller}
        exporting={exporting}
        importing={importing}
        jobs={jobs}
        onClose={() => {
          setExporting(false);
          setImporting(null);
        }}
      />
      <DiagramSheet
        diagram={diagram}
        profileId={connections.active?.id}
        controller={controller}
        onClose={() => {
          setDiagram(null);
        }}
      />
      <WorkspaceEditors connections={connections} controller={controller} />
      {controller.linkSheet}
      <ConnectionSheets
        connections={connections}
        onFlow={controller.windowTabs.openFlow}
      />
      <SshPrompts />
    </div>
  );
}

/** The ER diagram of a schema, opening tables in the workspace. */
function DiagramSheet(props: {
  diagram: DiagramRequest | null;
  profileId: string | undefined;
  controller: ReturnType<typeof useWorkspaceController>;
  onClose: () => void;
}): React.JSX.Element | null {
  const { diagram, profileId, controller } = props;
  if (!diagram || !profileId || diagram.profileId !== profileId) return null;
  return (
    <ErDiagram
      key={JSON.stringify([profileId, diagram.database, diagram.schema])}
      {...diagram}
      profileId={profileId}
      onOpen={(name) => {
        controller.openObject({
          database: diagram.database,
          schema: diagram.schema,
          name,
          kind: 'table',
          arguments: null,
        });
      }}
      onClose={props.onClose}
    />
  );
}

/** Export, import and Run SQL File… sheets. */
function TransferSheets(props: {
  connections: ReturnType<typeof useConnections>;
  controller: ReturnType<typeof useWorkspaceController>;
  exporting: boolean;
  importing: ImportTarget | null;
  jobs: JobActions;
  onClose: () => void;
}): React.JSX.Element {
  const { connections, controller } = props;
  return (
    <>
      <JobSheets actions={props.jobs} />
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

/** The open object editor (new table, edit view, drop schema, …). */
function WorkspaceEditors({
  connections,
  controller,
}: {
  connections: ReturnType<typeof useConnections>;
  controller: ReturnType<typeof useWorkspaceController>;
}): React.JSX.Element | null {
  const { objects, catalog } = controller;
  if (!objects.editor || !connections.active) return null;
  return (
    <ObjectEditors
      request={objects.editor}
      profile={connections.active}
      opened={catalog.opened}
      onCloseDatabase={catalog.closeDatabase}
      onClose={objects.close}
      onDone={objects.done}
    />
  );
}

function WorkspaceOverlay({
  controller,
  connections,
}: {
  controller: ReturnType<typeof useWorkspaceController>;
  connections: ReturnType<typeof useConnections>;
}): React.JSX.Element | null {
  const { mode, setMode, catalog, command } = controller;
  if (!mode) return null;
  return (
    <Suspense fallback={null}>
      <WorkspacePalettes
        key={mode}
        mode={mode}
        connections={connections}
        onSavedQuery={(sql, title) => {
          setMode(null);
          if (controller.scope)
            controller.tabs.open(controller.scope, { sql, title });
        }}
        onProjectQuery={(path) => {
          setMode(null);
          if (controller.scope)
            openQueryFile(controller.tabs, controller.scope, path).catch(
              reportProjectError,
            );
        }}
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
        onNewDatabase={
          controller.objects.features?.databases
            ? () => {
                setMode(null);
                controller.objects.open({
                  kind: 'database',
                  database: catalog.database,
                  name: null,
                });
              }
            : undefined
        }
      />
    </Suspense>
  );
}
