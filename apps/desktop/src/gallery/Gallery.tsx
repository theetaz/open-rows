// SPDX-License-Identifier: Apache-2.0
import { GlassPanel, SegmentedControl, useTheme } from '@openrows/ui';
import type { ThemeMode } from '@openrows/ui';
import { ButtonGallery } from './ButtonGallery';
import { EditorGallery } from './EditorGallery';
import { FieldGallery } from './FieldGallery';
import { IndicatorGallery } from './IndicatorGallery';
import { TokenGallery } from './TokenGallery';
import { AppFooter } from '../shell/AppFooter';
import styles from './Gallery.module.css';
import { t } from './messages';

const themes: readonly { value: ThemeMode; label: string }[] = [
  { value: 'system', label: t('system') },
  { value: 'light', label: t('light') },
  { value: 'dark', label: t('dark') },
];
const sections = [
  t('controls'),
  t('fields'),
  t('editor'),
  t('indicators'),
  t('tokens'),
];
/** Inspectable design-system gallery; no workspace/database behaviour. */
export function Gallery(): React.JSX.Element {
  return (
    <div className={styles.layout}>
      <GlassPanel className={styles.sidebar}>
        <nav aria-label={t('gallerySections')}>
          <strong>{t('componentGallery')}</strong>
          {sections.map((name) => (
            <a key={name} href={`#${name.toLowerCase()}`}>
              {name}
            </a>
          ))}
        </nav>
        <p className={styles.secondary}>{t('systemTypeConcentric')}</p>
      </GlassPanel>
      <div className={styles.content}>
        <GalleryHeader />
        <ButtonGallery />
        <FieldGallery />
        <EditorGallery />
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
        <p className={styles.eyebrow}>{t('openrowsDesignSystem')}</p>
        <h1>{t('madeDesktop')}</h1>
        <p className={styles.secondary}>{t('reusableControlsThoughtful')}</p>
      </div>
      <SegmentedControl
        aria-label={t('appearance')}
        options={themes}
        value={mode}
        onValueChange={setMode}
      />
    </div>
  );
}
