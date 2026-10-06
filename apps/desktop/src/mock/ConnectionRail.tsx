// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import { createPortal } from 'react-dom';
import { EnvBadge, Icon, IconButton, useReducedMotion } from '@openrows/ui';
import { connections } from './data/connections';
import type { Connection } from './data/connections';
import { workspaceMonogram } from './monogram';
import styles from './Rail.module.css';
import tooltipStyles from './Tooltip.module.css';
/** Workspace-only navigation; databases belong to the sidebar header. */
export function ConnectionRail(props: {
  selected: number;
  onSelect: (index: number) => void;
  onGallery: () => void;
}): React.JSX.Element {
  return (
    <nav className={styles.rail} aria-label="Workspaces">
      {connections.map((connection, index) => (
        <WorkspaceTile
          key={connection.name}
          connection={connection}
          selected={props.selected === index}
          onSelect={() => {
            props.onSelect(index);
          }}
        />
      ))}
      <div className={styles.space} />
      <IconButton aria-label="New connection" variant="ghost">
        <Icon name="plus" />
      </IconButton>
      <IconButton aria-label="API client" variant="ghost">
        API
      </IconButton>
      <IconButton
        aria-label="Component gallery"
        variant="ghost"
        onClick={props.onGallery}
      >
        <Icon name="settings" />
      </IconButton>
    </nav>
  );
}
function WorkspaceTile(props: {
  connection: Connection;
  selected: boolean;
  onSelect: () => void;
}): React.JSX.Element {
  const tooltip = useTileTooltip();
  const reduced = useReducedMotion();
  const { connection, selected } = props;
  return (
    <div className={styles.item}>
      <button
        type="button"
        className={styles.connection}
        aria-label={`Switch to ${connection.name}`}
        aria-pressed={selected}
        aria-describedby={tooltip.visible ? tooltip.id : undefined}
        onClick={props.onSelect}
        onKeyDown={navigateRail}
        {...tooltip.events}
      >
        <span
          aria-hidden="true"
          className={styles.indicator}
          data-state={
            selected ? 'selected' : tooltip.visible ? 'hover' : 'idle'
          }
          data-motion={reduced ? 'reduced' : 'full'}
        />
        <span className={styles.tile} data-engine={connection.tint}>
          {workspaceMonogram(connection.name)}
          <i aria-hidden="true" data-environment={connection.environment} />
        </span>
      </button>
      {tooltip.visible && (
        <WorkspaceTooltip
          connection={connection}
          id={tooltip.id}
          position={tooltip.position}
        />
      )}
    </div>
  );
}
function WorkspaceTooltip({
  connection,
  id,
  position,
}: {
  connection: Connection;
  id: string;
  position: { left: number; top: number };
}): React.JSX.Element {
  return createPortal(
    <div
      role="tooltip"
      id={id}
      className={tooltipStyles.tooltip}
      style={position}
    >
      <strong>{connection.name}</strong>
      <EnvBadge environment={connection.environment} />
      <small>
        {connection.engine} · {connection.host}
      </small>
    </div>,
    document.body,
  );
}
function useTileTooltip(): {
  visible: boolean;
  id: string;
  position: { left: number; top: number };
  events: Pick<
    React.ButtonHTMLAttributes<HTMLButtonElement>,
    'onMouseEnter' | 'onMouseLeave' | 'onFocus' | 'onBlur'
  >;
} {
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [position, setPosition] = useState({ left: 0, top: 0 });
  const id = useId();
  function locate(element: HTMLElement): void {
    const bounds = element.getBoundingClientRect();
    setPosition({ left: bounds.right + 10, top: bounds.top });
  }
  return {
    visible: hovered || focused,
    id,
    position,
    events: {
      onMouseEnter: (event) => {
        locate(event.currentTarget);
        setHovered(true);
      },
      onMouseLeave: () => {
        setHovered(false);
      },
      onFocus: (event) => {
        locate(event.currentTarget);
        setFocused(true);
      },
      onBlur: () => {
        setFocused(false);
      },
    },
  };
}
function navigateRail(event: React.KeyboardEvent<HTMLButtonElement>): void {
  if (event.nativeEvent.isComposing) return;
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
  const buttons = Array.from(
    event.currentTarget
      .closest('nav')
      ?.querySelectorAll<HTMLButtonElement>('[aria-pressed]') ?? [],
  );
  const index = buttons.indexOf(event.currentTarget);
  const next =
    (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) %
    buttons.length;
  event.preventDefault();
  buttons[next]?.focus();
}
