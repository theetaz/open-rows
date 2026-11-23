// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import {
  Badge,
  EngineIcon,
  EnvBadge,
  Icon,
  IconButton,
  isEngineId,
  SplitPanel,
} from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { displayCombo, useBindings } from '../commands';
import { t as shell } from '../../shell/messages';
import { ConnectionHeader, sessionOf } from '../connections';
import type { Connections } from '../connections';
import { openQueryFile, rebindFiles } from '../query';
import type { QueryTabs, TabScope } from '../query';
import { reportProjectError } from '../projects';
import { openInspection } from '../inspection/target';
import { AttachDatabaseTools } from '../schema';
import { safeModeState } from '../security/policies';
import { sidebarRange } from './layout';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { WorkspaceRail } from './WorkspaceRail';
import type { useWorkspaceController } from './useWorkspaceController';
import styles from './Frame.module.css';
import { t } from './messages';

type Props = {
  connections: Connections;
  controller: ReturnType<typeof useWorkspaceController>;
};

/**
 * The edge-to-edge sidebar: the traffic lights sit in its top row, then the
 * connection pop-up and one list at a time: the connection list while the
 * pop-up is expanded (or nothing is connected), otherwise the
 * Items / Queries / History source list.
 */
export function SidebarColumn(props: Props): React.JSX.Element {
  const { connections, controller } = props;
  const [expanded, setExpanded] = useState(false);
  const region = useId();
  const active = connections.active;
  const list = expanded || !active;
  const collapse = (): void => {
    setExpanded(false);
  };
  return (
    <SplitPanel
      label={t('sidebar')}
      storageKey="openrows.mock.sidebar"
      range={sidebarRange}
      open={controller.sidebar}
      edge="right"
    >
      <div className={styles.sidebar}>
        <div className={styles.traffic} data-tauri-drag-region />
        <ConnectionPopup
          connections={connections}
          open={list}
          controls={region}
          onToggle={() => {
            setExpanded(!expanded);
          }}
        />
        <div
          id={region}
          className={styles.connections}
          hidden={!list}
          onKeyDown={(event) => {
            if (event.key === 'Escape' && !event.defaultPrevented) collapse();
          }}
        >
          <WorkspaceRail
            connections={connections}
            onChosen={collapse}
            onProcesses={controller.openProcesses}
            onMonitor={controller.openMonitor}
            onRunFile={controller.jobs.runFile}
            jobs={controller.jobs}
          />
        </div>
        <div className={styles.items} hidden={list}>
          <SidebarItems connections={connections} controller={controller} />
        </div>
        <SidebarFooter onCommand={controller.command} />
      </div>
    </SplitPanel>
  );
}

/** The environment tag: LOCAL, DEV, STAGING, PROD, or a custom label. */
function EnvTag({ env }: { env: ProfileDto['env'] }): React.JSX.Element {
  if (env.kind === 'custom')
    return <Badge className={styles.envTag}>{env.label ?? env.kind}</Badge>;
  return <EnvBadge environment={env.kind} className={styles.envTag} />;
}

/**
 * The connection header (prototype `.switcher`): engine tile, name with its
 * session dot, database · host, environment tag and ⌃⌄. It switches the
 * sidebar between the connection list and the object browser.
 */
function ConnectionPopup(props: {
  connections: Connections;
  open: boolean;
  controls: string;
  onToggle: () => void;
}): React.JSX.Element | null {
  const { active, database } = props.connections;
  if (!active) return null;
  const shown = database || active.database || '';
  const context = [
    active.name,
    shown || t('noDatabase'),
    safeModeState(active).label,
  ].join(' · ');
  const place = active.file?.path ?? active.host;
  return (
    <button
      type="button"
      className={styles.popup}
      aria-expanded={props.open}
      aria-controls={props.controls}
      aria-label={t('connectionsContext', { context })}
      title={context}
      onClick={props.onToggle}
    >
      {isEngineId(active.engine) ? (
        <EngineIcon engine={active.engine} size="md" />
      ) : (
        <span className={styles.engineFallback}>
          <Icon name="database" />
        </span>
      )}
      <span className={styles.popupLines}>
        <span className={styles.popupName}>
          <strong>{active.name}</strong>
          <span
            className={styles.stateDot}
            data-state={props.connections.statusOf(active.id).state}
            aria-hidden="true"
          />
        </span>
        <span className={styles.popupDatabase}>
          {[shown || t('noDatabase'), place].filter(Boolean).join(' · ')}
        </span>
      </span>
      <EnvTag env={active.env} />
      <Icon name="chevronUpDown" />
    </button>
  );
}

/** The sidebar's last row: New connection and Settings. */
function SidebarFooter(props: {
  onCommand: (id: string) => void;
}): React.JSX.Element {
  const bindings = useBindings();
  const keys = (id: string): string => {
    const combo = bindings[id];
    return combo ? displayCombo(combo) : '';
  };
  const shortcut = keys('new-connection');
  const settings = keys('settings');
  return (
    <div className={styles.footer}>
      <button
        type="button"
        className={styles.footerRow}
        title={shortcut ? `${shell('newConnection')}  ${shortcut}` : undefined}
        onClick={() => {
          props.onCommand('new-connection');
        }}
      >
        <Icon name="plus" />
        <span>{shell('newConnection')}</span>
        {shortcut && <kbd aria-hidden="true">{shortcut}</kbd>}
      </button>
      <IconButton
        aria-label={shell('settings')}
        title={
          settings ? `${shell('settings')}  ${settings}` : shell('settings')
        }
        size="sm"
        variant="ghost"
        onClick={() => {
          props.onCommand('settings');
        }}
      >
        <Icon name="settings" />
      </IconButton>
    </div>
  );
}

/** Object tree of the active database, saved queries and history. */
function SidebarItems({ connections, controller }: Props): React.JSX.Element {
  const { catalog, tabs, scope } = controller;
  return (
    <WorkspaceSidebar
      header={<ConnectionHeader connections={connections} />}
      schemaTools={
        <SchemaTools connections={connections} controller={controller} />
      }
      catalog={catalog}
      connections={connections}
      onOpen={controller.openObject}
      onInspect={(target) => {
        if (scope) openInspection(tabs, scope, target);
      }}
      onDiagram={controller.showDiagram}
      onImport={controller.importData}
      jobs={controller.jobs}
      objects={controller.objects}
      onOpenSql={(sql) => {
        controller.addQuery(sql);
      }}
      onSavedQuery={(query) => {
        if (scope) tabs.open(scope, { title: query.name, sql: query.sql });
      }}
      {...projectOpeners(tabs, scope)}
      onConnection={connections.select}
      sessionFor={(name) => {
        const profile = connections.active;
        const sessionId = profile
          ? sessionOf(connections.statusOf(profile.id), name)
          : null;
        return profile && sessionId
          ? { profileId: profile.id, sessionId }
          : null;
      }}
    />
  );
}

/** Tools beside the schema picker: the ER diagram and engine extras. */
function SchemaTools({ connections, controller }: Props): React.JSX.Element {
  const { catalog } = controller;
  return (
    <>
      <IconButton
        aria-label={t('showErDiagram2')}
        title={t('showErDiagram')}
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

/** Project queries open in the active scope; bound tabs follow moved files. */
function projectOpeners(
  tabs: QueryTabs,
  scope: TabScope | undefined,
): {
  onProjectQuery: (path: string) => void;
  onProjectMoved: (from: string, to: string) => void;
} {
  return {
    onProjectQuery: (path) => {
      if (!scope) {
        reportProjectError(t('openDatabaseOpen'));
        return;
      }
      openQueryFile(tabs, scope, path).catch(reportProjectError);
    },
    onProjectMoved: (from, to) => {
      rebindFiles(tabs, from, to);
    },
  };
}
