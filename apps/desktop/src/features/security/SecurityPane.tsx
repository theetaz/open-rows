// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { securitySettings } from './securitySettings';
import { useSecurity } from './useSecurity';
import styles from '../appearance/Appearance.module.css';
import { t } from './lazyMessages';
const usePreferences = securitySettings.useValue;
const INACTIVITY_MINUTES = [1, 5, 15, 30, 60];
/** Settings ▸ Security: OS-authenticated startup and inactivity locks. */
export function SecurityPane(): React.JSX.Element {
  const preferences = usePreferences();
  const status = useSecurity();
  return (
    <section aria-label={t('security')}>
      <h3>{t('security')}</h3>
      {status.notice && <p role="status">{status.notice}</p>}
      <div className={styles.settingsGroup}>
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
          <SelectField
            aria-label={t('lockAfterInactivity')}
            disabled={!status.available}
            value={String(preferences.lockAfterInactivityMinutes ?? '')}
            options={[
              { value: '', label: t('off') },
              ...INACTIVITY_MINUTES.map((minutes) => ({
                value: String(minutes),
                label: t('minutesMinutes', { minutes }),
              })),
            ]}
            onValueChange={(value) => {
              securitySettings.set({
                ...preferences,
                lockAfterInactivityMinutes: value ? Number(value) : null,
              });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
