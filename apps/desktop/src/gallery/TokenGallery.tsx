// SPDX-License-Identifier: Apache-2.0
import type { CSSProperties } from 'react';
import { typography } from '@openrows/ui/tokens';
import styles from './Gallery.module.css';

const colours = [
  'page',
  'window',
  'panel',
  'text',
  'secondary',
  'tertiary',
  'accent',
  'separator',
  'control-fill',
  'control-active',
  'success-bg',
  'warning-bg',
  'info-bg',
  'neutral-bg',
  'danger-bg',
];
/** Swatches always resolve CSS variables, including system and high-contrast modes. */
export function TokenGallery(): React.JSX.Element {
  return (
    <section
      id="tokens"
      aria-labelledby="tokens-heading"
      className={styles.section}
    >
      <h2 id="tokens-heading">Type & colour tokens</h2>
      <div className={styles.grid}>
        <TypeExamples />
        <div className={styles.swatches}>
          {colours.map((name) => (
            <div key={name}>
              <div
                aria-hidden="true"
                className={styles.swatch}
                style={{ '--swatch': `var(--or-${name})` } as CSSProperties}
              />
              <span>{name}</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function TypeExamples(): React.JSX.Element {
  return (
    <div className={styles.stack}>
      {Object.keys(typography)
        .filter((key) => key.startsWith('text-'))
        .map((key) => (
          <div key={key} style={{ fontSize: `var(--or-${key})` }}>
            OpenRows · {key.slice(5).replace('-', '.')} px
          </div>
        ))}
      <p className={styles.mono}>SF Mono · 0123456789</p>
      <div className={styles.row}>
        {(['regular', 'medium', 'semibold', 'bold'] as const).map((weight) => (
          <span
            key={weight}
            style={{ fontWeight: `var(--or-weight-${weight})` }}
          >
            {weight}
          </span>
        ))}
      </div>
    </div>
  );
}
