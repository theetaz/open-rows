// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, updateGeneral } from './general';
import { diagnosticsSettings, updateDiagnostics } from './diagnostics';
import { updateFeed } from './updateFeed';
import { UpdateControls } from './UpdateControls';
import styles from '../appearance/Appearance.module.css';

/** Update/startup preferences, with explicit off-by-default reporting consent. */
export function StartupSettings(): React.JSX.Element {
  const general = generalSettings.useValue();
  const diagnostics = diagnosticsSettings.useValue();
  const endpoint = updateFeed(general.updateChannel);
  return (
    <section aria-label="Startup and updates">
      <h3>Startup and updates</h3>
      <p role="status">
        {endpoint
          ? `Updates are available on the ${general.updateChannel} channel.`
          : `${general.updateChannel} update feed is not configured in this build.`}
      </p>
      <UpdateControls />
      <div className={styles.settingsGroup}>
        <SettingRow
          title="Update channel"
          description="Choose stable releases or early beta versions."
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
          description="Check for signed releases and show notices. Installation requires your approval."
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
