// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, updateGeneral } from './general';
import { diagnosticsSettings, updateDiagnostics } from './diagnostics';
import { updateFeed } from './updateFeed';
import { UpdateControls } from './UpdateControls';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyGeneral';

/** Update/startup preferences, with explicit off-by-default reporting consent. */
export function StartupSettings(): React.JSX.Element {
  const general = generalSettings.useValue();
  const diagnostics = diagnosticsSettings.useValue();
  const endpoint = updateFeed(general.updateChannel);
  return (
    <section aria-label={t('startupUpdates')}>
      <h3>{t('startupUpdates')}</h3>
      <p role="status">
        {endpoint
          ? t('updatesAvailableUpdatechannel', {
              updateChannel: general.updateChannel,
            })
          : t('updatechannelUpdateFeed', {
              updateChannel: general.updateChannel,
            })}
      </p>
      <UpdateControls />
      <div className={styles.settingsGroup}>
        <SettingRow
          title={t('updateChannel')}
          description={t('chooseStableReleases')}
        >
          <SelectField
            aria-label={t('updateChannel')}
            value={general.updateChannel}
            options={[
              { value: 'stable', label: t('stable') },
              { value: 'beta', label: t('beta') },
            ]}
            onValueChange={(updateChannel) => {
              updateGeneral({ updateChannel });
            }}
          />
        </SettingRow>
        <SettingRow
          title={t('updateNotifications')}
          description={t('checkSignedReleases')}
        >
          <Switch
            aria-label={t('updateNotifications')}
            checked={general.updateNotifications}
            onCheckedChange={(updateNotifications) => {
              updateGeneral({ updateNotifications });
            }}
          />
        </SettingRow>
        <SettingRow
          title={t('newTabBehaviour')}
          description={t('createQueryActive')}
        >
          <SelectField
            aria-label={t('newTabBehaviour')}
            value={general.newTab}
            options={[
              { value: 'query', label: t('newQuery') },
              { value: 'connection', label: t('connectionPicker') },
            ]}
            onValueChange={(newTab) => {
              updateGeneral({ newTab });
            }}
          />
        </SettingRow>
        <SettingRow
          title={t('crashReporting')}
          description={t('optCrashReports')}
        >
          <Switch
            aria-label={t('crashReporting')}
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
