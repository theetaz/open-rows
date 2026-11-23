// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { Button, Icon } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { useUpdateInstall } from './useUpdateInstall';
import styles from './UpdateNotice.module.css';
import { generalSettings } from './general';
import { checkUpdate, dismissUpdate, useUpdate } from './updater';
import { t } from './messages/lazyGeneral';

/** Signed updates require approval and an empty, disconnected workspace. */
export function UpdateNotice({
  canInstall,
}: {
  canInstall: boolean;
}): React.JSX.Element | null {
  const general = generalSettings.useValue();
  const state = useUpdate();
  const { installing, error, start } = useUpdateInstall(
    state.update,
    canInstall,
  );
  useUpdateChecks();
  if (!general.updateNotifications || !state.update) return null;
  return (
    <section className={styles.notice} aria-label={t('updateAvailable')}>
      <header className={styles.head}>
        <Icon name="jobs" aria-hidden="true" />
        <div>
          <h2 aria-hidden="true">{t('updateAvailable')}</h2>
          <p role="status">{state.message}</p>
        </div>
      </header>
      {!canInstall && <p className={styles.note}>{t('closeAllTabs')}</p>}
      {error && <p role="alert">{error}</p>}
      <details>
        <summary>{t('releaseNotes')}</summary>
        <pre>{state.update.body || 'No release notes provided.'}</pre>
      </details>
      <footer className={styles.actions}>
        <Button disabled={installing} onClick={dismissUpdate}>
          {t('later')}
        </Button>
        <Button
          variant="primary"
          disabled={!canInstall || installing}
          onClick={() => {
            void start();
          }}
        >
          {t('installRestart')}
        </Button>
      </footer>
      {installing && (
        <Sheet title={t('installingUpdate')} onClose={() => undefined}>
          <p role="status" tabIndex={0} data-initial-focus="true">
            {t('verifyingInstallingUpdate')}
          </p>
        </Sheet>
      )}
    </section>
  );
}

function useUpdateChecks(): void {
  const general = generalSettings.useValue();
  useEffect(() => {
    dismissUpdate();
    if (!general.updateNotifications) return undefined;
    void checkUpdate();
    const timer = window.setInterval(
      () => {
        void checkUpdate();
      },
      24 * 60 * 60 * 1000,
    );
    return () => {
      window.clearInterval(timer);
      dismissUpdate();
    };
  }, [general.updateChannel, general.updateNotifications]);
}
