// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl, Switch } from '@openrows/ui';
import { openLogs } from '../diagnostics';
import { SettingRow } from '../appearance/InterfaceSettings';
import { diagnosticsSettings, updateDiagnostics } from './diagnostics';
import type { LogLevel } from './diagnostics';
import styles from '../appearance/Appearance.module.css';

const LEVELS: readonly { value: LogLevel; label: string }[] = [
  { value: 'error', label: 'Error' },
  { value: 'warn', label: 'Warn' },
  { value: 'info', label: 'Info' },
  { value: 'debug', label: 'Debug' },
  { value: 'trace', label: 'Trace' },
];

/** Settings ▸ Diagnostics: the log level, the logs folder and the crash-report stub. */
export function DiagnosticsPane(): React.JSX.Element {
  const diagnostics = diagnosticsSettings.use();
  const [failure, setFailure] = useState<string | null>(null);
  return (
    <>
      <section aria-label="Logs">
        <h3>Logs</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Log level"
            description="What the log file records: daily files, seven kept. Logs never hold passwords or query text. OPENROWS_LOG overrides this, and a change applies at the next launch."
          >
            <SegmentedControl
              aria-label="Log level"
              options={LEVELS}
              value={diagnostics.logLevel}
              onValueChange={(logLevel) => {
                updateDiagnostics({ logLevel });
              }}
            />
          </SettingRow>
          <SettingRow
            title="Logs folder"
            description="Open the folder with the log files, for example to attach them to a bug report."
          >
            <Button
              onClick={() => {
                openLogs().catch((error: unknown) => {
                  setFailure(
                    error instanceof Error ? error.message : String(error),
                  );
                });
              }}
            >
              Open logs folder
            </Button>
          </SettingRow>
          {failure && <p role="alert">{failure}</p>}
        </div>
      </section>
      <section aria-label="Crash reports">
        <h3>Crash reports</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Send crash reports"
            description="Off. OpenRows sends no telemetry and no crash reports. The opt-in reporter is not built yet; this switch only stores your choice."
          >
            <Switch
              aria-label="Send crash reports"
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
