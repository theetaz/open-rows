// SPDX-License-Identifier: Apache-2.0
import { Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { securitySettings } from './securitySettings';
import { useSecurity } from './useSecurity';
const usePreferences = securitySettings.useValue;
/** Settings ▸ Security: OS-authenticated startup and inactivity locks. */
export function SecurityPane(): React.JSX.Element {
  const preferences = usePreferences();
  const status = useSecurity();
  return (
    <section aria-label="Security">
      <h3>Security</h3>
      {status.notice && <p role="status">{status.notice}</p>}
      <SettingRow
        title="Lock on startup"
        description="Require OS authentication before showing any data in any window."
      >
        <Switch
          aria-label="Lock on startup"
          disabled={!status.available}
          checked={preferences.lockOnStartup}
          onCheckedChange={(lockOnStartup) => {
            securitySettings.set({ ...preferences, lockOnStartup });
          }}
        />
      </SettingRow>
      <SettingRow
        title="Lock after inactivity"
        description="No user input in any window, including time in the background."
      >
        <select
          aria-label="Lock after inactivity"
          disabled={!status.available}
          value={preferences.lockAfterInactivityMinutes ?? ''}
          onChange={(e) => {
            securitySettings.set({
              ...preferences,
              lockAfterInactivityMinutes: e.target.value
                ? Number(e.target.value)
                : null,
            });
          }}
        >
          <option value="">Off</option>
          {[1, 5, 15, 30, 60].map((minutes) => (
            <option key={minutes} value={minutes}>
              {minutes} minutes
            </option>
          ))}
        </select>
      </SettingRow>
    </section>
  );
}
