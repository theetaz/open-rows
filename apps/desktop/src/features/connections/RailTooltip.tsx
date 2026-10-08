// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import { createPortal } from 'react-dom';
import type { ProfileDto } from '@openrows/ipc';
import { profileLocation } from './engines';
import { EnvironmentBadge } from './EnvironmentBadge';
import styles from './Tooltip.module.css';

type Position = { left: number; top: number };

/** Glass tooltip beside a rail tile: name, environment, engine, address and status. */
export function RailTooltip(props: {
  profile: ProfileDto;
  shortcut?: string | undefined;
  status: string;
  id: string;
  position: Position;
}): React.JSX.Element {
  const { profile } = props;
  return createPortal(
    <div
      role="tooltip"
      id={props.id}
      className={styles.tooltip}
      style={props.position}
    >
      <strong>{profile.name}</strong>
      <span className={styles.trailing}>
        <EnvironmentBadge env={profile.env} />
        {props.shortcut && <kbd>{props.shortcut}</kbd>}
      </span>
      <small>{profileLocation(profile)}</small>
      <small>{props.status}</small>
    </div>,
    document.body,
  );
}

/** Hover and focus visibility with the tile's on-screen position. */
export function useTileTooltip(): {
  visible: boolean;
  id: string;
  position: Position;
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
