// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Switch } from '@openrows/ui';
import { configureMonitorHistory, purgeMonitorHistory } from '@openrows/ipc';
import { monitorSettings } from '../settings/monitorSettings';
import { settingsText } from '../settings/settingsFile';
import { writeSettingsFile } from '@openrows/ipc';
import styles from './Monitor.module.css';
import { t } from './messages/lazyDashboard';

/** Explicit per-connection opt-in and deletion; disabling synchronously purges native history. */
export function HistoryControls(props: {
  profileId: string;
  onPurged?: () => void;
}): React.JSX.Element {
  const settings = monitorSettings.useValue();
  const id = props.profileId.toLowerCase();
  const enabled = settings.enabledProfiles.includes(id);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async (action: () => Promise<void>): Promise<void> => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await action();
      props.onPurged?.();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section aria-label={t('localMonitorHistory')}>
      <label>
        {t('keepLocalMonitor')}{' '}
        <Switch
          aria-label={t('keepLocalMonitor')}
          disabled={busy}
          checked={enabled}
          onCheckedChange={(checked) => {
            void run(async () => {
              await changeHistory({ settings, id, checked });
              setNotice(
                checked ? t('localHistoryEnabled') : t('historyDisabledPurged'),
              );
            });
          }}
        />
      </label>
      <p className={styles.note}>{t('offDefaultRedacted')}</p>
      <div className={styles.actions}>
        <Button
          size="sm"
          disabled={busy}
          onClick={() => {
            void run(async () => {
              await purgeMonitorHistory(id);
              setNotice(t('connectionHistoryPurged'));
            });
          }}
        >
          {t('purgeConnectionHistory')}
        </Button>
        <Button
          size="sm"
          disabled={busy}
          onClick={() => {
            void run(async () => {
              await purgeMonitorHistory(null);
              setNotice(t('allMonitorHistory'));
            });
          }}
        >
          {t('purgeAllMonitor')}
        </Button>
      </div>
      {notice && <p role="status">{notice}</p>}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}

async function changeHistory(options: {
  settings: { enabledProfiles: string[] };
  id: string;
  checked: boolean;
}): Promise<void> {
  const { settings, id, checked } = options;
  const next = {
    enabledProfiles: checked
      ? [...settings.enabledProfiles, id]
      : settings.enabledProfiles.filter((profile) => profile !== id),
  };
  await configureMonitorHistory(next.enabledProfiles);
  monitorSettings.set(next);
  try {
    await writeSettingsFile(settingsText());
  } catch (failure) {
    monitorSettings.set(settings);
    await configureMonitorHistory(settings.enabledProfiles);
    throw failure;
  }
}
