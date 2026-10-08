// SPDX-License-Identifier: Apache-2.0
import type { ButtonHTMLAttributes, Ref } from 'react';
import styles from './Controls.module.css';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /** Anchor for a menu or popover opened by this button. */
  ref?: Ref<HTMLButtonElement>;
  variant?: 'primary' | 'secondary' | 'ghost' | 'destructive';
  size?: 'sm' | 'md';
};
/** Capsule action with native keyboard activation and disabled semantics. */
export function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps): React.JSX.Element {
  return (
    <button
      type="button"
      {...props}
      className={`${styles.button ?? ''} ${className}`}
      data-variant={variant}
      data-size={size}
    />
  );
}
/** Icon-only action; its accessible name is required at compile time. */
export function IconButton(
  props: Omit<ButtonProps, 'aria-label'> & { 'aria-label': string },
): React.JSX.Element {
  return (
    <Button
      {...props}
      className={`${styles.iconButton ?? ''} ${props.className ?? ''}`}
    />
  );
}
