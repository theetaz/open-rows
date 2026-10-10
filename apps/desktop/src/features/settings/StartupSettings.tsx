// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, updateGeneral } from './general';
import { diagnosticsSettings, updateDiagnostics } from './diagnostics';
import { updateFeed } from './updateFeed';
import styles from '../appearance/Appearance.module.css';

/** Update/startup preferences, with explicit off-by-default reporting consent. */
export function StartupSettings(): React.JSX.Element {
  const general = generalSettings.use();
  const diagnostics = diagnosticsSettings.use();
  const endpoint = updateFeed(general.updateChannel);
  return (
    <section aria-label="Startup and updates">
      <h3>Startup and updates</h3>
      <p role="status">
        {endpoint
          ? `Signed ${general.updateChannel} manifest: ${endpoint}`
          : `${general.updateChannel} update feed is not configured in this build.`}
      </p>
      <div className={styles.settingsGroup}>
        <SettingRow
          title="Update channel"
          description="Stable or signed beta releases. The release feed must be configured before updates are available."
        >
          <SelectField
            aria-label="Update channel"
            value={general.updateChannel}
            options={[
              { value: 'stable', label: 'Stable' },
              { value: 'beta', label: 'Beta' },
            ]}
            onValueChange={(updateChannel) => {
              updateGeneral({ updateChannel });
            }}
          />
        </SettingRow>
        <SettingRow
          title="Update notifications"
          description="Allow notices when a signed release is available; updates require your approval."
        >
          <Switch
            aria-label="Update notifications"
            checked={general.updateNotifications}
            onCheckedChange={(updateNotifications) => {
              updateGeneral({ updateNotifications });
            }}
          />
        </SettingRow>
        <SettingRow
          title="New tab behaviour"
          description="Create a query in the active database or open the connection picker."
        >
          <SelectField
            aria-label="New tab behaviour"
            value={general.newTab}
            options={[
              { value: 'query', label: 'New query' },
              { value: 'connection', label: 'Connection picker' },
            ]}
            onValueChange={(newTab) => {
              updateGeneral({ newTab });
            }}
          />
        </SettingRow>
        <SettingRow
          title="Crash reporting"
          description="Opt in to crash reports. Off by default; this build stores consent and has no reporting backend."
        >
          <Switch
            aria-label="Crash reporting"
            checked={diagnostics.crashReports}
            onCheckedChange={(crashReports) => {
              updateDiagnostics({ crashReports });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
