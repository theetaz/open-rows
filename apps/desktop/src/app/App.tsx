// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useState } from 'react';
import { Button, ThemeProvider } from '@openrows/ui';
import { useWindowReady } from './useWindowReady';
const Gallery = lazy(() =>
  import('../gallery/Gallery').then((module) => ({ default: module.Gallery })),
);
import { Workspace } from '../mock/Workspace';
import styles from './App.module.css';
const AppearanceSettings = lazy(() =>
  import('../features/appearance/AppearanceSettings').then((module) => ({
    default: module.AppearanceSettings,
  })),
);

/** Workspace is the default; the design gallery remains one click away. */
export function App(): React.JSX.Element {
  const startupError = useWindowReady();
  const [gallery, setGallery] = useState(false);
  const [appearance, setAppearance] = useState(false);
  useEffect(() => {
    const open = (event: KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.key === ',') {
        event.preventDefault();
        setAppearance(true);
      }
    };
    window.addEventListener('keydown', open);
    return () => {
      window.removeEventListener('keydown', open);
    };
  }, []);
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
            onAppearance={() => {
              setAppearance(true);
            }}
            onGallery={() => {
              setGallery(true);
            }}
          />
        </div>
      )}
      {appearance && (
        <Suspense fallback={null}>
          <AppearanceSettings
            onClose={() => {
              setAppearance(false);
            }}
          />
        </Suspense>
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
            <Suspense fallback={null}>
              <Gallery />
            </Suspense>
          </main>
        </div>
      )}
    </ThemeProvider>
  );
}
