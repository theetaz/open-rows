// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Switch } from '@openrows/ui';
import { configureMonitorHistory, purgeMonitorHistory } from '@openrows/ipc';
import { monitorSettings } from '../settings/monitorSettings';
import { settingsText } from '../settings/settingsFile';
import { writeSettingsFile } from '@openrows/ipc';
import styles from './Monitor.module.css';

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
    <section aria-label="Local monitor history">
      <label>
        Keep local monitor history{' '}
        <Switch
          aria-label="Keep local monitor history"
          disabled={busy}
          checked={enabled}
          onCheckedChange={(checked) => {
            void run(async () => {
              await changeHistory({ settings, id, checked });
              setNotice(
                checked
                  ? 'Local history enabled.'
                  : 'History disabled and purged.',
              );
            });
          }}
        />
      </label>
      <p className={styles.note}>
        Off by default. Redacted scalar aggregates only; no SQL text or result
        rows. Retention 24 hours, 50 MiB per connection and 100 MiB app-wide.
        Oldest observations expire first. Collection stops when Monitor closes.
        The local store is not encrypted; database names may be sensitive.
      </p>
      <div className={styles.actions}>
        <Button
          size="sm"
          disabled={busy}
          onClick={() => {
            void run(async () => {
              await purgeMonitorHistory(id);
              setNotice('Connection history purged.');
            });
          }}
        >
          Purge connection history
        </Button>
        <Button
          size="sm"
          disabled={busy}
          onClick={() => {
            void run(async () => {
              await purgeMonitorHistory(null);
              setNotice('All monitor history purged.');
            });
          }}
        >
          Purge all monitor history
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
