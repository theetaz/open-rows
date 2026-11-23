// SPDX-License-Identifier: Apache-2.0
import type { CSSProperties } from 'react';
import { Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import type { EnvDto } from '@openrows/ipc';
import type { SafeModeState } from '../security/policies';
import type { ContextSegments } from './contextBar';
import { ConnectionStatus } from './ConnectionStatus';
import { environmentColour, InsecureBadge } from '../connections';
import { JobsButton, useJobsVisible } from '../jobs';
import { FavouriteButton } from '../schema/FavouriteButton';
import type { SidebarObject } from '../schema/sidebarIdentity';
import styles from './Workspace.module.css';
import { useWindowActive } from './useWindowActive';
import { t } from './messages';

type Props = {
  /** The launch-time documents control; `undefined` when there are none. */
  documents?: React.ReactNode;
  context: ContextSegments;
  /** The active tab's object, starred from the end of the context bar. */
  favourite?: SidebarObject | undefined;
  /** Connection state for the context bar's health dot. */
  health: string;
  onDatabase?: (() => void) | undefined;
  onPalette: () => void;
  onRefresh?: (() => void) | undefined;
  /** The active connection's real protection; `null` without a connection. */
  safeMode: SafeModeState | null;
  /** Open the connection's settings at its safety policy. */
  onSafeMode: () => void;
  sidebar: boolean;
  inspector: boolean;
  onSidebar: () => void;
  onInspector: () => void;
  onQuery: () => void;
};
/**
 * Unified toolbar: small glass capsules either side of an address capsule
 * sized to its content (TablePlus). Without a connection only the two panel
 * toggles remain.
 */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  useWindowActive();
  const connected = props.context.environment !== null;
  return (
    <header
      className={styles.toolbar}
      style={tint(props.context.environment)}
      data-sidebar-state={props.sidebar ? 'open' : 'closed'}
      data-tauri-drag-region
      aria-label={t('windowToolbar')}
    >
      <div className={styles.side} data-tauri-drag-region>
        <ToolbarGroup aria-label={t('sidebarControls')}>
          <ToolButton
            label={t('toggleSidebarS')}
            icon="sidebar"
            pressed={props.sidebar}
            onClick={props.onSidebar}
          />
        </ToolbarGroup>
        {connected && (
          <ToolbarGroup aria-label={t('safety')}>
            <SafeModeButton state={props.safeMode} onClick={props.onSafeMode} />
          </ToolbarGroup>
        )}
      </div>
      {connected ? <Context {...props} /> : <span />}
      <div
        className={[styles.side, styles.end].join(' ')}
        data-tauri-drag-region
      >
        {connected && (
          <ToolbarGroup aria-label={t('connectionTools')}>
            <ToolButton
              label={t('newSqlQuery')}
              icon="terminal"
              text={t('sql')}
              onClick={props.onQuery}
            />
            <ToolButton
              label={t('commandPalette')}
              icon="command"
              onClick={props.onPalette}
            />
          </ToolbarGroup>
        )}
        <WorkspaceActions documents={props.documents} />
        <ToolbarGroup aria-label={t('inspectorControls')}>
          <ToolButton
            label={t('toggleInspectorI')}
            icon="inspector"
            pressed={props.inspector}
            onClick={props.onInspector}
          />
        </ToolbarGroup>
      </div>
    </header>
  );
}
/** Jobs (only while there are some) and the launch-time documents. */
function WorkspaceActions({
  documents,
}: Pick<Props, 'documents'>): React.JSX.Element | null {
  const jobs = useJobsVisible();
  if (!jobs && !documents) return null;
  return (
    <ToolbarGroup aria-label={t('workspaceActions')}>
      {jobs && <JobsButton />}
      {documents}
    </ToolbarGroup>
  );
}
/**
 * Reports the saved safety policy and opens the setting that changes it: a
 * quiet open lock when unrestricted, a green lock and its word when on.
 */
function SafeModeButton({
  state,
  onClick,
}: {
  state: SafeModeState | null;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={
        state ? t('safeModeLabel', { label: state.label }) : t('safeMode')
      }
      aria-haspopup="dialog"
      title={
        state
          ? t('safeModeLabelDetail', {
              label: state.label,
              detail: state.detail,
            })
          : t('safeModeNot')
      }
      className={state?.on ? styles.labelled : styles.lock}
      data-on={state?.on ?? false}
      disabled={!state}
      variant="ghost"
      size="sm"
      onClick={onClick}
    >
      <Icon name={state?.on ? 'lock' : 'unlock'} />
      {state?.on && <span>{state.label}</span>}
    </IconButton>
  );
}
/** A toolbar action; the label doubles as its tooltip, `text` shows beside the icon. */
function ToolButton(props: {
  label: string;
  icon: IconName;
  text?: string;
  pressed?: boolean;
  onClick: () => void;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={props.label}
      className={props.text ? styles.labelled : undefined}
      aria-pressed={props.pressed}
      variant="ghost"
      size="sm"
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
      {props.text && <span>{props.text}</span>}
    </IconButton>
  );
}
/** Environments loud enough for the strip under the toolbar (FR-CONN-06). */
const tinted = new Set(['prod', 'staging', 'custom']);
/** The environment's colour for the strip under the toolbar and its tag. */
function tint(environment: EnvDto | null): CSSProperties | undefined {
  return environment && tinted.has(environment.kind)
    ? ({ '--env-color': environmentColour(environment) } as CSSProperties)
    : undefined;
}
/** `ENV ● connection › database › schema.object ☆ ⟳`, left aligned. */
function Context({
  context,
  health,
  favourite,
  onDatabase,
  onRefresh,
}: Pick<
  Props,
  'context' | 'health' | 'favourite' | 'onDatabase' | 'onRefresh'
>): React.JSX.Element {
  const { env, engine, name, database, object } = context;
  const address = [engine, name, database, object].filter(Boolean).join(' : ');
  return (
    <div
      className={styles.context}
      aria-label={t('connectionContext')}
      title={env ? `${env} | ${address}` : address}
    >
      {env && (
        <span className={styles.envTag} data-env={context.environment?.kind}>
          {env}
        </span>
      )}
      <ConnectionStatus health={health} route={context.route} />
      <InsecureBadge reasons={context.insecure} />
      <Address context={context} onDatabase={onDatabase} />
      {favourite && (
        <FavouriteButton object={favourite} className={styles.star} />
      )}
      <IconButton
        aria-label={t('refresh')}
        title={t('refresh')}
        className={styles.refresh}
        disabled={!onRefresh}
        variant="ghost"
        size="sm"
        onClick={onRefresh}
      >
        <Icon name="refresh" />
      </IconButton>
    </div>
  );
}
/**
 * The address parts. Each separator belongs to the part after it, so a part
 * that shrinks away takes its `›` along; the connection name never shrinks.
 */
function Address({
  context,
  onDatabase,
}: Pick<Props, 'context' | 'onDatabase'>): React.JSX.Element {
  const { name, database, object } = context;
  return (
    <span className={styles.address}>
      <strong data-part="name">{name}</strong>
      <span className={styles.segment} data-part="database">
        <Separator />
        <button
          className={styles.databaseSegment}
          type="button"
          aria-label={t('openDatabaseDatabase', {
            database: database || t('noDatabaseName'),
          })}
          onClick={onDatabase}
          disabled={!onDatabase}
        >
          {database || t('noDatabase')}
        </button>
      </span>
      {object && (
        <span className={styles.segment} data-part="object">
          <Separator />
          <span className={styles.text}>{object}</span>
        </span>
      )}
    </span>
  );
}
function Separator(): React.JSX.Element {
  return (
    <span className={styles.sep} aria-hidden="true">
      ›
    </span>
  );
}
