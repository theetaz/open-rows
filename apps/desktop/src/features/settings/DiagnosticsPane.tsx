// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl, Switch } from '@openrows/ui';
import { openLogs } from '../diagnostics';
import { SettingRow } from '../appearance/InterfaceSettings';
import { diagnosticsSettings, updateDiagnostics } from './diagnostics';
import type { LogLevel } from './diagnostics';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyGeneral';

const LEVELS: readonly { value: LogLevel; label: string }[] = [
  { value: 'error', label: t('error') },
  { value: 'warn', label: t('warn') },
  { value: 'info', label: t('info') },
  { value: 'debug', label: t('debug') },
  { value: 'trace', label: t('trace') },
];

/** Settings ▸ Diagnostics: the log level, the logs folder and the crash-report stub. */
export function DiagnosticsPane(): React.JSX.Element {
  const diagnostics = diagnosticsSettings.useValue();
  const [failure, setFailure] = useState<string | null>(null);
  return (
    <>
      <section aria-label={t('logs')}>
        <h3>{t('logs')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow title={t('logLevel')} description={t('whatLogFile')}>
            <SegmentedControl
              aria-label={t('logLevel')}
              options={LEVELS}
              value={diagnostics.logLevel}
              onValueChange={(logLevel) => {
                updateDiagnostics({ logLevel });
              }}
            />
          </SettingRow>
          <SettingRow title={t('logsFolder')} description={t('openFolderLog')}>
            <Button
              onClick={() => {
                openLogs().catch((error: unknown) => {
                  setFailure(
                    error instanceof Error ? error.message : String(error),
                  );
                });
              }}
            >
              {t('openLogsFolder')}
            </Button>
          </SettingRow>
          {failure && <p role="alert">{failure}</p>}
        </div>
      </section>
      <section aria-label={t('crashReports')}>
        <h3>{t('crashReports')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title={t('sendCrashReports')}
            description={t('offOpenrowsSends')}
          >
            <Switch
              aria-label={t('sendCrashReports')}
              checked={diagnostics.crashReports}
              onCheckedChange={(crashReports) => {
                updateDiagnostics({ crashReports });
              }}
            />
          </SettingRow>
        </div>
      </section>
    </>
  );
}
