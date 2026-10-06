// SPDX-License-Identifier: Apache-2.0
import { useAppInfo } from './useAppInfo';
import styles from './AppFooter.module.css';

/** Small native metadata footer with explicit IPC loading and error states. */
export function AppFooter(): React.JSX.Element {
  const state = useAppInfo();
  return (
    <footer className={styles.footer} aria-label="Application information">
      {state.status === 'loading' && (
        <p role="status">Loading application information…</p>
      )}
      {state.status === 'error' && (
        <p role="alert">
          Unable to load application information: {state.message}
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
