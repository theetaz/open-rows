// SPDX-License-Identifier: Apache-2.0
import { useAppInfo } from './useAppInfo';
import styles from './AppFooter.module.css';
import { t } from './messages';

/** Small native metadata footer with explicit IPC loading and error states. */
export function AppFooter(): React.JSX.Element {
  const state = useAppInfo();
  return (
    <footer className={styles.footer} aria-label={t('applicationInformation')}>
      {state.status === 'loading' && (
        <p role="status">{t('loadingApplicationInformation')}</p>
      )}
      {state.status === 'error' && (
        <p role="alert">
          {t('unableLoadApplication', { message: state.message })}
        </p>
      )}
      {state.status === 'success' && (
        <p>
          {state.info.name} {state.info.version} · {state.info.buildProfile}
        </p>
      )}
    </footer>
  );
}
