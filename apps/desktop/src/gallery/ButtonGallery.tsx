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
      <h2 id="controls-heading">Controls</h2>
      <p className={styles.secondary}>
        Hover any action for an inset edge. Tab to see the focus ring; Space or
        Enter activates.
      </p>
      <div className={styles.stack}>
        {variants.map((variant) => (
          <ActionExamples key={variant} variant={variant} />
        ))}
      </div>
      <ToolbarExamples />
      <GlassPanel>
        <strong>GlassPanel</strong>
        <p className={styles.secondary}>
          30 px blur, a bright inset highlight, and 18 px corners. Reduce
          Transparency replaces glass with a solid surface.
        </p>
      </GlassPanel>
    </section>
  );
}
function ActionExamples({ variant }: { variant: Variant }): React.JSX.Element {
  return (
    <div>
      <div className={styles.row}>
        <span className={styles.label}>{variant}</span>
        <Button variant={variant}>{variant} · md</Button>
        <Button variant={variant} size="sm">
          Small
        </Button>
        <Button variant={variant} disabled>
          Disabled
        </Button>
        <Button variant={variant} className={styles.focusPreview}>
          Focus specimen
        </Button>
      </div>
      <IconExamples variant={variant} />
    </div>
  );
}
const iconStates = [
  { label: 'default md', size: 'md', disabled: false, focus: false },
  { label: 'default sm', size: 'sm', disabled: false, focus: false },
  { label: 'disabled', size: 'md', disabled: true, focus: false },
  { label: 'focus', size: 'md', disabled: false, focus: true },
] as const;
function IconExamples({ variant }: { variant: Variant }): React.JSX.Element {
  return (
    <div className={styles.row}>
      <span className={styles.label}>IconButton</span>
      {iconStates.map((state) => (
        <IconButton
          key={state.label}
          aria-label={`${variant} icon ${state.label}`}
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
      <ToolbarGroup aria-label="Gallery actions">
        <IconButton aria-label="Add item">
          <Icon name="plus" />
        </IconButton>
        <IconButton aria-label="Refresh">
          <Icon name="refresh" />
        </IconButton>
        <IconButton aria-label="Toggle sidebar" disabled>
          <Icon name="sidebar" />
        </IconButton>
        <IconButton
          aria-label="Focused icon specimen"
          className={styles.focusPreview}
        >
          <Icon name="search" />
        </IconButton>
      </ToolbarGroup>
      <span>ToolbarGroup · IconButton</span>
      <Kbd>⌘ K</Kbd>
    </div>
  );
}
