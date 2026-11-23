// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import type { McpSettings } from '@openrows/ipc';
import { Icon, TextField } from '@openrows/ui';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
/** Explicit local bridge/HTTP choices and OS-selected port by default. */
export function McpPreferences(props: {
  settings: Required<McpSettings>;
  busy: boolean;
  change: (settings: Required<McpSettings>) => void;
  /** The server state row, first in the group. */
  status: ReactNode;
}): React.JSX.Element {
  const { settings, busy, change } = props;
  return (
    <fieldset disabled={busy} className={styles.fieldset}>
      <legend>{t('mcpServer')}</legend>
      <div className={[styles.group, styles.withIcons].join(' ')}>
        {props.status}
        <label className={styles.row}>
          <span className={styles.rowIcon}>
            <Icon name="terminal" />
          </span>
          <span className={styles.rowBody}>{t('enableMcpStdio')}</span>
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(event) => {
              change({ ...settings, enabled: event.target.checked });
            }}
          />
        </label>
        <label className={styles.row}>
          <span className={styles.rowIcon}>
            <Icon name="database" />
          </span>
          <span className={styles.rowBody}>{t('allowDirectLocalhost')}</span>
          <input
            type="checkbox"
            checked={settings.httpEnabled}
            onChange={(event) => {
              change({ ...settings, httpEnabled: event.target.checked });
            }}
          />
        </label>
        <div className={styles.row}>
          <span className={styles.rowIcon}>
            <Icon name="settings" />
          </span>
          <TextField
            className={styles.inlineField}
            label={t('mcpPort0')}
            type="number"
            min={0}
            max={65535}
            controlSize="compact"
            value={String(settings.port)}
            onChange={(event) => {
              const port = Number(event.target.value);
              if (Number.isInteger(port) && port >= 0 && port <= 65535)
                change({ ...settings, port });
            }}
          />
        </div>
      </div>
      <p className={styles.foot}>{t('aiFeaturesMust')}</p>
    </fieldset>
  );
}
