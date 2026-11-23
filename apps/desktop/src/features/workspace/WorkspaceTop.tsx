// SPDX-License-Identifier: Apache-2.0
import type { CSSProperties } from 'react';
import type { EnvDto } from '@openrows/ipc';
import { NativeDocuments } from '../query/NativeDocuments';
import { WindowTabStrip } from '../query/WindowTabStrip';
import { environmentColour, insecureReasons } from '../connections';
import type { Connections } from '../connections';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { PathBar } from './PathBar';
import { PathTools } from './PathTools';
import { tabFavourite } from './contextBar';
import { routeStatus } from './routeStatus';
import type { useWorkspaceController } from './useWorkspaceController';
import styles from './Workspace.module.css';

type Props = {
  connections: Connections;
  controller: ReturnType<typeof useWorkspaceController>;
};

/** Environments loud enough for the strip across the window top (FR-CONN-06). */
const tinted = new Set(['prod', 'staging', 'custom']);

function strip(environment: EnvDto | undefined): CSSProperties | undefined {
  return environment && tinted.has(environment.kind)
    ? ({ '--env-color': environmentColour(environment) } as CSSProperties)
    : undefined;
}

/** The launch-time documents control, only while such documents are open. */
function documents(
  tabs: Props['controller']['tabs'],
): React.JSX.Element | undefined {
  return tabs.tabsFor({ connectionId: 'local-files', database: 'Files' })
    .length ? (
    <NativeDocuments tabs={tabs} />
  ) : undefined;
}

/** The path bar of the shown tab, with its tools. */
function Path({ connections, controller }: Props): React.JSX.Element {
  const { catalog, windowTabs: tabs } = controller;
  const profile = connections.active;
  const status = profile ? connections.statusOf(profile.id) : undefined;
  const flow = tabs.shown?.kind === 'connection';
  return (
    <PathBar
      connections={connections}
      catalog={catalog}
      tab={tabs.shown}
      onNewConnection={() => {
        tabs.openFlow({ mode: 'new' });
      }}
      onObject={(object) => {
        controller.openObject(object);
      }}
      onNavigate={(id, place) => controller.tabs.navigate?.(id, place)}
      tools={
        profile &&
        status &&
        !flow && (
          <PathTools
            health={status.state}
            route={routeStatus(profile)}
            insecure={insecureReasons(profile, status)}
            safeMode={controller.safeMode}
            onSafeMode={controller.openSafety}
            favourite={tabFavourite(tabs.shown, profile.id)}
            onRefresh={
              status.state === 'connected' ? controller.refresh : undefined
            }
          />
        )
      }
    />
  );
}

/**
 * The window's top: the environment strip, the tab strip beside the traffic
 * lights, and the toolbar with the path bar under it (DESIGN.md §6, §7).
 */
export function WorkspaceTop(props: Props): React.JSX.Element {
  const { connections, controller } = props;
  const { windowTabs: tabs, inspector, setMode } = controller;
  const shown = tabs.shown;
  const history = shown?.kind === 'table' ? shown : undefined;
  return (
    <div className={styles.top} style={strip(connections.active?.env)}>
      <WindowTabStrip
        tabs={controller.tabs}
        list={tabs.list}
        active={shown?.id}
        profiles={connections.profiles}
        sidebar={controller.sidebar ? 'open' : 'closed'}
        onSelect={tabs.select}
        onAdd={() => {
          controller.command('new-tab');
        }}
        onAddRight={tabs.addRight}
        onMove={(id, target) => tabs.host.get()?.onMove?.(id, target)}
        onReload={(id) => {
          const refresh = (): void => {
            controller.command('refresh');
          };
          if (id === shown?.id) refresh();
          else {
            tabs.select(id);
            // The tab's view takes the table commands once it has rendered.
            requestAnimationFrame(() => requestAnimationFrame(refresh));
          }
        }}
      />
      <WorkspaceToolbar
        path={<Path {...props} />}
        connected={Boolean(connections.active)}
        documents={documents(controller.tabs)}
        onPalette={() => {
          setMode('commands');
        }}
        sidebar={controller.sidebar}
        inspector={inspector.open}
        onSidebar={controller.toggleSidebar}
        onInspector={inspector.toggle}
        onQuery={() => {
          controller.command('new-tab');
        }}
        onBack={
          history?.back?.length
            ? () => controller.tabs.navigate?.(history.id, 'back')
            : null
        }
        onForward={
          history?.forward?.length
            ? () => controller.tabs.navigate?.(history.id, 'forward')
            : null
        }
      />
    </div>
  );
}
