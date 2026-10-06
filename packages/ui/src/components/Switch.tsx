// SPDX-License-Identifier: Apache-2.0
import type { ButtonHTMLAttributes } from 'react';
import styles from './Controls.module.css';

type Props = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'onChange' | 'role' | 'aria-checked'
> & {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  'aria-label': string;
};
/** Native button activation supplies Space/Enter for a controlled ARIA switch. */
export function Switch({
  checked,
  onCheckedChange,
  className = '',
  ...props
}: Props): React.JSX.Element {
  return (
    <button
      type="button"
      {...props}
      role="switch"
      aria-checked={checked}
      className={`${styles.switch ?? ''} ${className}`}
      onClick={() => {
        onCheckedChange(!checked);
      }}
    >
      <span className={styles.thumb} />
    </button>
  );
}
