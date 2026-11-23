// SPDX-License-Identifier: Apache-2.0
import { useRef } from 'react';
import { placeFloating } from '../../shell/menuPlacement';
import styles from './ConnectionStatus.module.css';
import { t } from './messages';

/**
 * Native light-dismiss status popover under the toolbar health dot, its left
 * edge just before the dot's, clamped like every menu.
 */
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
        aria-label={t('connectionStatus')}
        aria-haspopup="dialog"
        onClick={(event) => {
          const popover = panel.current;
          if (!popover) return;
          const anchor = event.currentTarget.getBoundingClientRect();
          popover.togglePopover();
          if (popover.matches(':popover-open'))
            placeFloating(popover, {
              x: anchor.left - 12,
              y: anchor.bottom + 4,
              flip: anchor.top - 4,
            });
        }}
      >
        <span aria-hidden="true">●</span>
      </button>
      <div
        ref={panel}
        popover="auto"
        className={styles.panel}
        role="dialog"
        aria-label={t('connectionStatusDetails')}
      >
        <p>{t('statusHealth', { health })}</p>
        <p>{route}</p>
      </div>
    </>
  );
}
