// SPDX-License-Identifier: Apache-2.0
import { Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { securitySettings } from './securitySettings';
import { useSecurity } from './useSecurity';
import { t } from './lazyMessages';
const usePreferences = securitySettings.useValue;
/** Settings ▸ Security: OS-authenticated startup and inactivity locks. */
export function SecurityPane(): React.JSX.Element {
  const preferences = usePreferences();
  const status = useSecurity();
  return (
    <section aria-label={t('security')}>
      <h3>{t('security')}</h3>
      {status.notice && <p role="status">{status.notice}</p>}
      <SettingRow
        title={t('lockStartup')}
        description={t('requireOsAuthentication')}
      >
        <Switch
          aria-label={t('lockStartup')}
          disabled={!status.available}
          checked={preferences.lockOnStartup}
          onCheckedChange={(lockOnStartup) => {
            securitySettings.set({ ...preferences, lockOnStartup });
          }}
        />
      </SettingRow>
      <SettingRow
        title={t('lockAfterInactivity')}
        description={t('noUserInput')}
      >
        <select
          aria-label={t('lockAfterInactivity')}
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
          <option value="">{t('off')}</option>
          {[1, 5, 15, 30, 60].map((minutes) => (
            <option key={minutes} value={minutes}>
              {t('minutesMinutes', { minutes })}
            </option>
          ))}
        </select>
      </SettingRow>
    </section>
  );
}
