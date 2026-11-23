// SPDX-License-Identifier: Apache-2.0
import { Switch, TextField } from '@openrows/ui';
import { historySettings, parseHistorySettings } from './historySettings';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyPanes';
/** Settings ▸ General ▸ History; saved changes sweep stored history. */
export function HistorySection(): React.JSX.Element {
  const settings = historySettings.useValue();
  return (
    <section aria-label={t('historyRetention')}>
      <h3>{t('history')}</h3>
      <div className={styles.settingsGroup}>
        <label className={styles.fieldRow}>
          {t('keepForever')}{' '}
          <Switch
            aria-label={t('keepHistoryForever')}
            checked={settings.keepForever}
            onCheckedChange={(keepForever) => {
              historySettings.set({ ...settings, keepForever });
            }}
          />
        </label>
        <TextField
          className={styles.fieldRow}
          label={t('maximumEntriesPer')}
          type="number"
          min={1}
          max={4294967295}
          disabled={settings.keepForever}
          value={String(settings.maxEntries)}
          onChange={(event) => {
            historySettings.set(
              parseHistorySettings({
                ...settings,
                maxEntries: Number(event.target.value),
              }),
            );
          }}
        />
        <TextField
          className={styles.fieldRow}
          label={t('maximumAgeDays')}
          type="number"
          min={1}
          max={4294967295}
          disabled={settings.keepForever}
          value={String(settings.maxAgeDays)}
          onChange={(event) => {
            historySettings.set(
              parseHistorySettings({
                ...settings,
                maxAgeDays: Number(event.target.value),
              }),
            );
          }}
        />
      </div>
      <p>{t('expiredEntriesRemoved')}</p>
    </section>
  );
}
