// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import type { InputHTMLAttributes, ReactNode } from 'react';
import styles from './TextField.module.css';

/** Regular (28 px) or compact (24 px) control size; see docs/20 §3.3. */
export type ControlSize = 'regular' | 'compact';
type Name =
  | { label: string; 'aria-label'?: string }
  | { label?: never; 'aria-label': string };
type Props = Omit<InputHTMLAttributes<HTMLInputElement>, 'aria-label'> &
  Name & {
    leadingIcon?: ReactNode;
    /** Accessory inside the frame after the input, such as a reveal button. */
    trailing?: ReactNode;
    error?: string;
    controlSize?: ControlSize | 'search';
  };
/** Labelled input with optional original icon and associated validation message. */
export function TextField({
  label,
  leadingIcon,
  trailing,
  error,
  controlSize = 'regular',
  id,
  className = '',
  ...props
}: Props): React.JSX.Element {
  const generated = useId();
  const inputId = id ?? generated;
  const errorId = `${inputId}-error`;
  const describedBy =
    [props['aria-describedby'], error ? errorId : undefined]
      .filter(Boolean)
      .join(' ') || undefined;
  return (
    <div className={`${styles.field ?? ''} ${className}`}>
      {label && <label htmlFor={inputId}>{label}</label>}
      <div
        className={styles.frame}
        data-invalid={Boolean(error)}
        data-size={controlSize}
      >
        {leadingIcon && (
          <span aria-hidden="true" className={styles.icon}>
            {leadingIcon}
          </span>
        )}
        <input
          {...props}
          id={inputId}
          aria-invalid={Boolean(error)}
          aria-describedby={describedBy}
        />
        {trailing && <span className={styles.trailing}>{trailing}</span>}
      </div>
      {error && (
        <span id={errorId} className={styles.error}>
          {error}
        </span>
      )}
    </div>
  );
}
