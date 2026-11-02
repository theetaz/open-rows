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
/** Overlay titlebar right of the workspace sidebar; the context bar flexes. */
export function WorkspaceToolbar(props: Props): React.JSX.Element {
  useWindowActive();
  return (
    <header
      className={styles.toolbar}
      style={tint(props.context)}
      data-tauri-drag-region
      aria-label="Window toolbar"
    >
      <ToolbarGroup aria-label="Sidebar controls">
        <ToolButton
          label="Toggle sidebar (⌃⌘S)"
          icon="sidebar"
          pressed={props.sidebar}
          onClick={props.onSidebar}
        />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Navigation">
        <ToolButton label="Back" icon="chevronLeft" />
        <ToolButton label="Forward" icon="chevronRight" />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Connection tools">
        <SafeModeButton state={props.safeMode} onClick={props.onSafeMode} />
        <ToolButton
          label="Open database (⌘K)"
          icon="database"
          onClick={props.onDatabase}
        />
        <ToolButton
          label="New SQL query"
          icon="terminal"
          onClick={props.onQuery}
        />
      </ToolbarGroup>
      <Context
        context={props.context}
        health={props.health}
        favourite={props.favourite}
        onDatabase={props.onDatabase}
      />
      <ToolbarGroup aria-label="Workspace actions">
        <ToolButton label="Refresh" icon="refresh" onClick={props.onRefresh} />
        <JobsButton />
        {props.documents}
        <ToolButton
          label="Command palette"
          icon="command"
          onClick={props.onPalette}
        />
      </ToolbarGroup>
      <ToolbarGroup aria-label="Inspector controls">
        <ToolButton
          label="Toggle inspector (⌥⌘I)"
          icon="inspector"
          pressed={props.inspector}
          onClick={props.onInspector}
        />
      </ToolbarGroup>
    </header>
  );
}
/**
 * Reports the saved safety policy and opens the setting that changes it. A
 * toggle here could only guess which of the six policies "off" should mean.
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
      aria-label={state ? `Safe mode: ${state.label}` : 'Safe mode'}
      aria-haspopup="dialog"
      title={
        state
          ? `Safe mode: ${state.label}. ${state.detail} Click to change it in connection settings.`
          : 'Safe mode is not available without a connection.'
      }
      className={styles.safe}
      data-on={state?.on ?? false}
      disabled={!state}
      variant="ghost"
      size="sm"
      onClick={onClick}
    >
      <Icon name={state?.on ? 'lock' : 'unlock'} />
    </IconButton>
  );
}
/** Icon-only toolbar action; the label doubles as its tooltip. */
function ToolButton(props: {
  label: string;
  icon: IconName;
  pressed?: boolean;
  onClick?: (() => void) | undefined;
}): React.JSX.Element {
  return (
    <IconButton
      aria-label={props.label}
      title={
        props.onClick
          ? props.label
          : `${props.label} is not available for this connection.`
      }
      disabled={!props.onClick}
      aria-pressed={props.pressed}
      variant="ghost"
      size="sm"
      onClick={props.onClick}
    >
      <Icon name={props.icon} />
    </IconButton>
  );
}
/** The environment's colour for the strip under the toolbar and its tag. */
function tint({ environment }: ContextSegments): CSSProperties | undefined {
  return environment
    ? ({ '--env-color': environmentColour(environment) } as CSSProperties)
    : undefined;
}
/** `ENV | PostgreSQL 14.17 : connection : database : schema.object`. */
function Context({
  context,
  health,
  favourite,
  onDatabase,
}: Pick<
  Props,
  'context' | 'health' | 'favourite' | 'onDatabase'
>): React.JSX.Element {
  const { env, engine, name, database, object } = context;
  const address = [engine, name, database, object].filter(Boolean).join(' : ');
  const title = env ? `${env} | ${address}` : address;
  return (
    <div
      className={styles.context}
      data-glass-rim="address"
      aria-label="Connection context"
      title={title}
    >
      <ConnectionStatus health={health} route={context.route} />
      {env && (
        <span className={styles.env}>
          <span className={styles.envTag} data-env={context.environment?.kind}>
            {env}
          </span>
          <span className={styles.bar} aria-hidden="true">
            {' | '}
          </span>
        </span>
      )}
      <InsecureBadge reasons={context.insecure} />
      <span className={styles.address}>
        <span>
          {engine} : {name}
        </span>
        {name !== 'No connection' && (
          <>
            <span aria-hidden="true">{' : '}</span>
            <button
              className={styles.databaseSegment}
              type="button"
              aria-label={`Open database (${database || 'none'})`}
              onClick={onDatabase}
              disabled={!onDatabase}
            >
              {database || 'No database'}
            </button>
          </>
        )}
        {object && <span>{` : ${object}`}</span>}
      </span>
      {favourite && (
        <FavouriteButton object={favourite} className={styles.star} />
      )}
    </div>
  );
}
