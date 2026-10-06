// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, ThemeProvider } from '@openrows/ui';
import { useWindowReady } from './useWindowReady';
import { Gallery } from '../gallery/Gallery';
import { Workspace } from '../mock/Workspace';
import styles from './App.module.css';

/** Workspace is the default; the design gallery remains one click away. */
export function App(): React.JSX.Element {
  const startupError = useWindowReady();
  const [gallery, setGallery] = useState(false);
  useEffect(() => {
    if (!gallery) return;
    const escape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') setGallery(false);
    };
    window.addEventListener('keydown', escape);
    return () => {
      window.removeEventListener('keydown', escape);
    };
  }, [gallery]);
  return (
    <ThemeProvider>
      {startupError ? (
        <p role="alert">Unable to show the native window.</p>
      ) : (
        <div hidden={gallery}>
          <Workspace
            enabled={!gallery}
            onGallery={() => {
              setGallery(true);
            }}
          />
        </div>
      )}
      {gallery && (
        <div className={styles.shell}>
          <header
            className={styles.toolbar}
            data-tauri-drag-region
            aria-label="Gallery toolbar"
          >
            <Button
              size="sm"
              onClick={() => {
                setGallery(false);
              }}
            >
              Back to workspace
            </Button>
          </header>
          <main className={styles.content}>
            <Gallery />
          </main>
        </div>
      )}
    </ThemeProvider>
  );
}
