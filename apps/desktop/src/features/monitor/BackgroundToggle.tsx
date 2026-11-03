// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { setMonitorBackground } from '@openrows/ipc';
import type { MonitorTarget } from '@openrows/ipc';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

/** Explicit per-connection opt-in; collection continues only while OpenRows is open and unlocked. */
export function BackgroundToggle(props: {
  target: MonitorTarget;
  enabled: boolean;
  disabled: boolean;
}): React.JSX.Element {
  const [pending, setPending] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const checked = pending ?? props.enabled;
  return (
    <>
      <label title={t('keepMonitorAlert')}>
        <input
          type="checkbox"
          checked={checked}
          disabled={props.disabled}
          onChange={(event) => {
            const enabled = event.target.checked;
            setPending(enabled);
            setMonitorBackground(props.target, enabled).then(
              () => {
                setPending(null);
                setError(null);
              },
              (failure: unknown) => {
                setPending(null);
                setError(String(failure));
              },
            );
          }}
        />
        {t('background')}
      </label>
      {error && (
        <span role="alert" className={styles.error}>
          {error}
        </span>
      )}
    </>
  );
}
