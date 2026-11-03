// SPDX-License-Identifier: Apache-2.0
import {
  Button,
  IconButton,
  Icon,
  ToolbarGroup,
  Kbd,
  GlassPanel,
} from '@openrows/ui';
import styles from './Gallery.module.css';
import { t } from './messages';

const variants = ['primary', 'secondary', 'ghost', 'destructive'] as const;
type Variant = (typeof variants)[number];
/** Action variants and reproducible visual-state specimens. */
export function ButtonGallery(): React.JSX.Element {
  return (
    <section
      id="controls"
      aria-labelledby="controls-heading"
      className={styles.section}
    >
      <h2 id="controls-heading">{t('controls')}</h2>
      <p className={styles.secondary}>{t('hoverAnyAction')}</p>
      <div className={styles.stack}>
        {variants.map((variant) => (
          <ActionExamples key={variant} variant={variant} />
        ))}
      </div>
      <ToolbarExamples />
      <GlassPanel>
        <strong>{t('glasspanel')}</strong>
        <p className={styles.secondary}>{t('n30PxBlur')}</p>
      </GlassPanel>
    </section>
  );
}
function ActionExamples({ variant }: { variant: Variant }): React.JSX.Element {
  return (
    <div>
      <div className={styles.row}>
        <span className={styles.label}>{variant}</span>
        <Button variant={variant}>{t('variantMd', { variant })}</Button>
        <Button variant={variant} size="sm">
          {t('small')}
        </Button>
        <Button variant={variant} disabled>
          {t('disabled')}
        </Button>
        <Button variant={variant} className={styles.focusPreview}>
          {t('focusSpecimen')}
        </Button>
      </div>
      <IconExamples variant={variant} />
    </div>
  );
}
const iconStates = [
  { label: t('defaultMd'), size: 'md', disabled: false, focus: false },
  { label: t('defaultSm'), size: 'sm', disabled: false, focus: false },
  { label: t('disabled2'), size: 'md', disabled: true, focus: false },
  { label: t('focus'), size: 'md', disabled: false, focus: true },
] as const;
function IconExamples({ variant }: { variant: Variant }): React.JSX.Element {
  return (
    <div className={styles.row}>
      <span className={styles.label}>{t('iconbutton')}</span>
      {iconStates.map((state) => (
        <IconButton
          key={state.label}
          aria-label={t('variantIconLabel', { variant, label: state.label })}
          variant={variant}
          size={state.size}
          disabled={state.disabled}
          className={state.focus ? styles.focusPreview : undefined}
        >
          <Icon name="plus" />
        </IconButton>
      ))}
    </div>
  );
}
function ToolbarExamples(): React.JSX.Element {
  return (
    <div className={styles.row}>
      <ToolbarGroup aria-label={t('galleryActions')}>
        <IconButton aria-label={t('addItem')}>
          <Icon name="plus" />
        </IconButton>
        <IconButton aria-label={t('refresh')}>
          <Icon name="refresh" />
        </IconButton>
        <IconButton aria-label={t('toggleSidebar')} disabled>
          <Icon name="sidebar" />
        </IconButton>
        <IconButton
          aria-label={t('focusedIconSpecimen')}
          className={styles.focusPreview}
        >
          <Icon name="search" />
        </IconButton>
      </ToolbarGroup>
      <span>{t('toolbargroupIconbutton')}</span>
      <Kbd>{t('k')}</Kbd>
    </div>
  );
}
