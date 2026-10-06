// SPDX-License-Identifier: Apache-2.0
import { useWindowReady } from './useWindowReady';
import { ThemeProvider } from '@openrows/ui';
import { Gallery } from '../gallery/Gallery';
import styles from './App.module.css';

/** Native gallery shell with traffic-light-safe, control-free drag region. */
export function App(): React.JSX.Element {
  const startupError = useWindowReady();
  return (
    <ThemeProvider>
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
            <Gallery />
          )}
        </main>
      </div>
    </ThemeProvider>
  );
}
