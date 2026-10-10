// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useState } from 'react';
import { Button, ThemeProvider } from '@openrows/ui';
import { LockBoundary } from '../features/security/LockBoundary';
import { useWindowReady } from './useWindowReady';
const Gallery = lazy(() =>
  import('../gallery/Gallery').then((module) => ({ default: module.Gallery })),
);
import { Workspace } from '../features/workspace';
import { commandForEvent } from '../features/commands';
import { useCrashRecovery } from '../features/diagnostics';
import {
  applyGeneral,
  generalSettings,
  settingsChanged,
  useSettingsRevision,
} from '../features/settings';
import type { SettingsSection } from '../features/settings';
import styles from './App.module.css';
const SettingsSheet = lazy(() =>
  import('../features/settings/SettingsSheet').then((module) => ({
    default: module.SettingsSheet,
  })),
);
const CrashRecoverySheet = lazy(() =>
  import('../features/diagnostics/CrashRecoverySheet').then((module) => ({
    default: module.CrashRecoverySheet,
  })),
);

/** The Settings shortcut (⌘, by default) opens Settings over any view. */
function useSettingsKey(open: () => void): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (event.defaultPrevented || event.isComposing) return;
      if (commandForEvent(event, ['settings']) !== 'settings') return;
      event.preventDefault();
      open();
    };
    window.addEventListener('keydown', listener);
    return () => {
      window.removeEventListener('keydown', listener);
    };
  }, [open]);
}

/** Density and reduced motion follow Settings ▸ General. */
function useGeneralSettings(): void {
  const general = generalSettings.use();
  useEffect(() => {
    applyGeneral(general);
  }, [general]);
}

/** Workspace is the default; the design gallery remains one click away. */
export function App(): React.JSX.Element {
  const startupError = useWindowReady();
  const [gallery, setGallery] = useState(false);
  const [settings, setSettings] = useState<SettingsSection | null>(null);
  const crash = useCrashRecovery();
  useSettingsKey(() => {
    setSettings((open) => open ?? 'general');
  });
  useGeneralSettings();
  const revision = useSettingsRevision();
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
    <ThemeProvider onChange={settingsChanged} revision={revision}>
      <LockBoundary>
        {startupError ? (
          <p role="alert">Unable to show the native window.</p>
        ) : (
          <div hidden={gallery}>
            <Workspace
              enabled={!gallery}
              onSettings={(section) => {
                setSettings(section ?? 'general');
              }}
              onGallery={() => {
                setGallery(true);
              }}
            />
          </div>
        )}
        {settings && (
          <Suspense fallback={null}>
            <SettingsSheet
              initial={settings}
              onClose={() => {
                setSettings(null);
              }}
            />
          </Suspense>
        )}
        {crash.crashed && (
          <Suspense fallback={null}>
            <CrashRecoverySheet onClose={crash.dismiss} />
          </Suspense>
        )}
        {gallery && <GalleryView onBack={setGallery} />}
      </LockBoundary>
    </ThemeProvider>
  );
}

function GalleryView(props: {
  onBack: (open: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.shell}>
      <header
        className={styles.toolbar}
        data-tauri-drag-region
        aria-label="Gallery toolbar"
      >
        <Button
          size="sm"
          onClick={() => {
            props.onBack(false);
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
  );
}
