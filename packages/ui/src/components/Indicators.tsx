// SPDX-License-Identifier: Apache-2.0
import type { HTMLAttributes } from 'react';
import styles from './Indicators.module.css';

type Tone = 'warning' | 'success' | 'info' | 'neutral' | 'danger';
type Environment = 'local' | 'dev' | 'staging' | 'prod';
/** Semantic status capsule; a text label always accompanies colour. */
export function Badge({
  tone = 'neutral',
  className = '',
  ...props
}: HTMLAttributes<HTMLSpanElement> & { tone?: Tone }): React.JSX.Element {
  return (
    <span
      {...props}
      data-tone={tone}
      className={`${styles.badge ?? ''} ${className}`}
    />
  );
}
/**
 * Environment tag (LOCAL, DEV, STAGING, PROD): an uppercase capsule in the
 * environment colour; production is solid red. The word carries the meaning.
 */
export function EnvBadge({
  environment,
  className = '',
}: {
  environment: Environment;
  className?: string | undefined;
}): React.JSX.Element {
  return (
    <span className={`${styles.env ?? ''} ${className}`} data-env={environment}>
      {environment}
    </span>
  );
}
/** Labelled status dot suitable for connection health. */
export function StatusDot({
  tone,
  label,
}: {
  tone: Tone;
  label: string;
}): React.JSX.Element {
  return (
    <span className={styles.status} data-tone={tone}>
      <span className={styles.dot} aria-hidden="true" />
      {label}
    </span>
  );
}
/** Native shortcut hint: keycaps in palettes, inline glyphs in action buttons. */
export function Kbd({
  variant = 'keycap',
  ...props
}: HTMLAttributes<HTMLElement> & {
  variant?: 'keycap' | 'inline';
}): React.JSX.Element {
  return <kbd {...props} data-variant={variant} className={styles.kbd} />;
}
