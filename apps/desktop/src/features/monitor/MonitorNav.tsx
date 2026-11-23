// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';
import { Icon } from '@openrows/ui';
import type { Panel, PanelOption } from './monitorPanels';
import styles from './MonitorLayout.module.css';
import { t } from './messages/lazyDashboard';

function nextIndex(key: string, index: number, count: number): number | null {
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  if (key === 'ArrowDown' || key === 'ArrowRight') return (index + 1) % count;
  if (key === 'ArrowUp' || key === 'ArrowLeft')
    return (index - 1 + count) % count;
  return null;
}

/**
 * The Monitor's section list (the kit's source list): one 26 px row per
 * section with its icon, the current one on `accent-soft`. It stays a
 * single-select radio group with wrapping arrows, Home/End and roving focus.
 */
export function MonitorNav(props: {
  options: readonly PanelOption[];
  value: Panel;
  /** Open incidents, shown as a count on the Incidents row. */
  openIncidents: number;
  onValueChange: (value: Panel) => void;
}): React.JSX.Element {
  const navigate = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const index = props.options.findIndex((o) => o.value === props.value);
    const next = nextIndex(event.key, index, props.options.length);
    const option = next === null ? undefined : props.options[next];
    if (!option || next === null) return;
    event.preventDefault();
    props.onValueChange(option.value);
    const peers =
      event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>(
        '[role="radio"]',
      );
    peers?.[next]?.focus();
  };
  return (
    <nav className={styles.nav}>
      <div className={styles.navLabel} aria-hidden="true">
        {t('monitor')}
      </div>
      <div role="radiogroup" aria-label={t('monitorPanel')}>
        {props.options.map((option) => (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={option.value === props.value}
            tabIndex={option.value === props.value ? 0 : -1}
            className={styles.navItem}
            onClick={() => {
              props.onValueChange(option.value);
            }}
            onKeyDown={navigate}
          >
            <Icon name={option.icon} />
            <span>{option.label}</span>
            {option.value === 'incidents' && props.openIncidents > 0 && (
              <span className={styles.count} aria-hidden="true">
                {props.openIncidents}
              </span>
            )}
          </button>
        ))}
      </div>
    </nav>
  );
}
