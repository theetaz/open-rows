// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { unlockApp } from '@openrows/ipc';
import styles from './Security.module.css';
import { t } from './lazyMessages';
/** Opaque, accessible lock screen with OS-owned credentials. */
export function LockScreen(): React.JSX.Element {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const unlock = (): void => {
    setBusy(true);
    setError(null);
    unlockApp()
      .catch((e: unknown) => {
        setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        setBusy(false);
      });
  };
  return (
    <main className={styles.lock} aria-label={t('openrowsLocked')}>
      <h1>{t('openrowsLocked2')}</h1>
      <p>{t('useTouchId')}</p>
      <Button autoFocus disabled={busy} onClick={unlock}>
        {busy ? t('authenticating') : t('unlockOpenrows')}
      </Button>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
