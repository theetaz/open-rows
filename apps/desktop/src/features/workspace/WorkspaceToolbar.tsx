// SPDX-License-Identifier: Apache-2.0
import { ButtonGroup, Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import { JobsButton, useJobsVisible } from '../jobs';
import { AgentActivityButton } from '../apps/AgentActivityButton';
import styles from './Workspace.module.css';
import { useWindowActive } from './useWindowActive';
import { t } from './messages';

type Props = {
  /** The launch-time documents control; `undefined` when there are none. */
  documents?: React.ReactNode;
  /** The path bar of the shown tab. */
  path: React.ReactNode;
  /** A connection is selected: the query and palette capsule shows. */
  connected: boolean;
  onPalette: () => void;
  sidebar: boolean;
  inspector: boolean;
  onSidebar: () => void;
  onInspector: () => void;
  onQuery: () => void;
  /** The shown tab's history, or `null` for none. */
  onBack: (() => void) | null;
  onForward: (() => void) | null;
};

/** A capsule action; the label doubles as its tooltip, `text` shows beside the icon. */
function ToolButton(props: {
  label: string;
  icon: IconName;
  text?: string;
  pressed?: boolean;
  disabled?: boolean;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={props.label}
      className={props.text ? styles.labelled : undefined}
      aria-pressed={props.pressed}
      disabled={props.disabled}
      variant="ghost"
      size="toolbar"
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
      {props.text && <span>{props.text}</span>}
    </IconButton>
  );
}

/** External agent activity, jobs (only while there are some) and the launch-time documents. */
function WorkspaceActions({
  documents,
}: Pick<Props, 'documents'>): React.JSX.Element {
  const jobs = useJobsVisible();
  return (
    <>
      <AgentActivityButton />
      {(jobs || documents) && (
        <ToolbarGroup aria-label={t('workspaceActions')}>
          {jobs && <JobsButton />}
          {documents}
        </ToolbarGroup>
      )}
    </>
  );
}

/**
 * The toolbar under the tab strip (DESIGN.md §6): sidebar and history, the
 * path bar filling the row, the query and palette capsule, activity, and
 * the inspector. Every control is a 32 px capsule on one centre line.
 */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  useWindowActive();
  return (
    <header
      className={styles.toolbar}
      data-tauri-drag-region
      aria-label={t('windowToolbar')}
    >
      <ButtonGroup size="toolbar" aria-label={t('sidebarControls')}>
        <ToolButton
          label={t('toggleSidebarS')}
          icon="sidebar"
          pressed={props.sidebar}
          onClick={props.onSidebar}
        />
        <ToolButton
          label={`${t('back')} (⌃⌘←)`}
          icon="chevronLeft"
          disabled={!props.onBack}
          onClick={() => props.onBack?.()}
        />
        <ToolButton
          label={`${t('forward')} (⌃⌘→)`}
          icon="chevronRight"
          disabled={!props.onForward}
          onClick={() => props.onForward?.()}
        />
      </ButtonGroup>
      <div className={styles.path}>{props.path}</div>
      {props.connected && (
        <ButtonGroup size="toolbar" aria-label={t('connectionTools')}>
          <ToolButton
            label={t('newSqlQuery')}
            icon="terminal"
            text={t('query')}
            onClick={props.onQuery}
          />
          <ToolButton
            label={t('commandPalette')}
            icon="command"
            onClick={props.onPalette}
          />
        </ButtonGroup>
      )}
      <WorkspaceActions documents={props.documents} />
      <ButtonGroup size="toolbar" aria-label={t('inspectorControls')}>
        <ToolButton
          label={t('toggleInspectorI')}
          icon="inspector"
          pressed={props.inspector}
          onClick={props.onInspector}
        />
      </ButtonGroup>
    </header>
  );
}
