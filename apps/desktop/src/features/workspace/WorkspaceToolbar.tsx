// SPDX-License-Identifier: Apache-2.0
import type { CSSProperties } from 'react';
import { Icon, IconButton, ToolbarGroup } from '@openrows/ui';
import type { IconName } from '@openrows/ui';
import type { SafeModeState } from '../security/policies';
import type { ContextSegments } from './contextBar';
import { ConnectionStatus } from './ConnectionStatus';
import { environmentColour, InsecureBadge } from '../connections';
import { JobsButton } from '../jobs';
import { FavouriteButton } from '../schema/FavouriteButton';
import type { SidebarObject } from '../schema/sidebarIdentity';
import styles from './Workspace.module.css';
import { useWindowActive } from './useWindowActive';
import { t } from './messages';

type Props = {
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
 * Unified toolbar: small glass capsules either side of a centred address
 * capsule (TablePlus), with the safety lock and its policy on the left.
 */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  useWindowActive();
  return (
    <header
      className={styles.toolbar}
      style={tint(props.context)}
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
        <ToolbarGroup aria-label={t('navigation')}>
          <ToolButton label={t('back')} icon="chevronLeft" />
          <ToolButton label={t('forward')} icon="chevronRight" />
        </ToolbarGroup>
        <ToolbarGroup aria-label={t('safety')}>
          <SafeModeButton state={props.safeMode} onClick={props.onSafeMode} />
        </ToolbarGroup>
      </div>
      <Context {...props} />
      <div
        className={[styles.side, styles.end].join(' ')}
        data-tauri-drag-region
      >
        <ToolbarGroup aria-label={t('connectionTools')}>
          <ToolButton
            label={t('newSqlQuery')}
            icon="terminal"
            text={t('sql')}
            onClick={props.onQuery}
          />
          <ToolButton
            label={t('openDatabaseK')}
            icon="database"
            onClick={props.onDatabase}
          />
        </ToolbarGroup>
        <ToolbarGroup aria-label={t('workspaceActions')}>
          <JobsButton />
          {props.documents}
          <ToolButton
            label={t('commandPalette')}
            icon="command"
            onClick={props.onPalette}
          />
        </ToolbarGroup>
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
/**
 * Reports the saved safety policy (a lock and its word) and opens the setting
 * that changes it. A toggle here could only guess which policy "off" means.
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
      className={styles.labelled}
      data-on={state?.on ?? false}
      disabled={!state}
      variant="ghost"
      size="sm"
      onClick={onClick}
    >
      <Icon name={state?.on ? 'lock' : 'unlock'} />
      {state && <span>{state.label}</span>}
    </IconButton>
  );
}
/** A toolbar action; the label doubles as its tooltip, `text` shows beside the icon. */
function ToolButton(props: {
  label: string;
  icon: IconName;
  text?: string;
  pressed?: boolean;
  onClick?: (() => void) | undefined;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={
        props.onClick
          ? props.label
          : t('labelNotAvailable', { label: props.label })
      }
      className={props.text ? styles.labelled : undefined}
      disabled={!props.onClick}
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
/** The environment's colour for the strip under the toolbar and its tag. */
function tint({ environment }: ContextSegments): CSSProperties | undefined {
  return environment
    ? ({ '--env-color': environmentColour(environment) } as CSSProperties)
    : undefined;
}
/** `ENV | PostgreSQL 14.17 : connection : database : schema.object`, centred. */
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
  const title = env ? `${env} | ${address}` : address;
  return (
    <div
      className={styles.context}
      aria-label={t('connectionContext')}
      title={title}
    >
      <ConnectionStatus health={health} route={context.route} />
      <InsecureBadge reasons={context.insecure} />
      <Address context={context} onDatabase={onDatabase} />
      {favourite && (
        <FavouriteButton object={favourite} className={styles.star} />
      )}
      <IconButton
        aria-label={t('refresh')}
        title={
          onRefresh
            ? t('refresh')
            : t('labelNotAvailable', { label: t('refresh') })
        }
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
/** The address parts; they shrink by importance, the connection name never. */
function Address({
  context,
  onDatabase,
}: Pick<Props, 'context' | 'onDatabase'>): React.JSX.Element {
  const { env, engine, name, database, object } = context;
  return (
    <span className={styles.address}>
      {env && (
        <span className={styles.env} data-part="env">
          <span className={styles.envTag} data-env={context.environment?.kind}>
            {env}
          </span>
          <span className={styles.sep} aria-hidden="true">
            {' | '}
          </span>
        </span>
      )}
      <span data-part="engine">{engine}</span>
      <span className={styles.sep}> : </span>
      <strong data-part="name">{name}</strong>
      {name !== t('noConnection') && (
        <>
          <span className={styles.sep} aria-hidden="true">
            {' : '}
          </span>
          <button
            className={styles.databaseSegment}
            data-part="database"
            type="button"
            aria-label={t('openDatabaseDatabase', {
              database: database || t('noDatabaseName'),
            })}
            onClick={onDatabase}
            disabled={!onDatabase}
          >
            {database || t('noDatabase')}
          </button>
        </>
      )}
      {object && (
        <>
          <span className={styles.sep}> : </span>
          <span data-part="object">{object}</span>
        </>
      )}
    </span>
  );
}
