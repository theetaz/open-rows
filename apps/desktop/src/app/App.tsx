// SPDX-License-Identifier: Apache-2.0
import { useWindowReady } from './useWindowReady';
import { InfoCard } from '../shell/InfoCard';
import styles from './App.module.css';

/** Native window shell with a traffic-light-safe draggable toolbar. */
export function App(): React.JSX.Element {
  const startupError = useWindowReady();

  return (
    <div className={styles.shell}>
      <header
        className={styles.toolbar}
        data-tauri-drag-region
        aria-label="Window toolbar"
      >
        <span data-tauri-drag-region>OpenRows</span>
      </header>
      <main className={styles.content}>
        {startupError ? (
          <p role="alert">Unable to show the native window.</p>
        ) : (
          <InfoCard />
        )}
      </main>
    </div>
  );
}
