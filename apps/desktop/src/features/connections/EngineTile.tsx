// SPDX-License-Identifier: Apache-2.0
import styles from './EnginePicker.module.css';

/** Neutral round tile with the engine's two-letter monogram (no vendor logos). */
export function EngineTile({
  monogram,
  size = 'large',
}: {
  monogram: string;
  size?: 'large' | 'small';
}): React.JSX.Element {
  return (
    <span className={styles.tile} data-size={size} aria-hidden="true">
      {monogram}
    </span>
  );
}
