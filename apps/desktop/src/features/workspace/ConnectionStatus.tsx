// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import styles from './ConnectionStatus.module.css';

/** Native light-dismiss status popover anchored to the toolbar health dot. */
export function ConnectionStatus({
  health,
  route,
}: {
  health: string;
  route: string;
}): React.JSX.Element {
  const panel = useRef<HTMLDivElement>(null);
  return (
    <>
      <button
        type="button"
        className={styles.health}
        data-state={health}
        aria-label="Connection status"
        aria-haspopup="dialog"
        onClick={(event) => {
          const popover = panel.current;
          if (!popover) return;
          const anchor = event.currentTarget.getBoundingClientRect();
          popover.style.left = `${String(Math.max(8, anchor.right - 320))}px`;
          popover.style.top = `${String(anchor.bottom + 4)}px`;
          popover.togglePopover();
        }}
      >
        <span aria-hidden="true">●</span>
      </button>
      <div
        ref={panel}
        popover="auto"
        className={styles.panel}
        role="dialog"
        aria-label="Connection status details"
      >
        <p>Status: {health}</p>
        <p>{route}</p>
      </div>
    </>
  );
}
