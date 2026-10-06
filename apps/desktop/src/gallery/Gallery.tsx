// SPDX-License-Identifier: Apache-2.0
import { GlassPanel, SegmentedControl, useTheme } from '@openrows/ui';
import type { ThemeMode } from '@openrows/ui';
import { ButtonGallery } from './ButtonGallery';
import { FieldGallery } from './FieldGallery';
import { IndicatorGallery } from './IndicatorGallery';
import { TokenGallery } from './TokenGallery';
import { AppFooter } from '../shell/AppFooter';
import styles from './Gallery.module.css';

const themes: readonly { value: ThemeMode; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];
const sections = ['Controls', 'Fields', 'Indicators', 'Tokens'];
/** Inspectable design-system gallery; no workspace/database behaviour. */
export function Gallery(): React.JSX.Element {
  return (
    <div className={styles.layout}>
      <GlassPanel className={styles.sidebar}>
        <nav aria-label="Gallery sections">
          <strong>Component gallery</strong>
          {sections.map((name) => (
            <a key={name} href={`#${name.toLowerCase()}`}>
              {name}
            </a>
          ))}
        </nav>
        <p className={styles.secondary}>
          System type. Concentric corners. Glass materials.
        </p>
      </GlassPanel>
      <div className={styles.content}>
        <GalleryHeader />
        <ButtonGallery />
        <FieldGallery />
        <IndicatorGallery />
        <TokenGallery />
        <AppFooter />
      </div>
    </div>
  );
}

function GalleryHeader(): React.JSX.Element {
  const { mode, setMode } = useTheme();
  return (
    <div className={styles.intro}>
      <div>
        <p className={styles.eyebrow}>OPENROWS / DESIGN SYSTEM</p>
        <h1>Made for the desktop.</h1>
        <p className={styles.secondary}>
          Reusable controls, thoughtful defaults.
        </p>
      </div>
      <SegmentedControl
        aria-label="Appearance"
        options={themes}
        value={mode}
        onValueChange={setMode}
      />
    </div>
  );
}
