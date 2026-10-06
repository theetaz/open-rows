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
/** Named environment pill; never communicates deployment risk by colour alone. */
export function EnvBadge({
  environment,
}: {
  environment: Environment;
}): React.JSX.Element {
  return (
    <span className={styles.badge} data-tone={environment}>
      <span className={styles.dot} aria-hidden="true" />
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
/** Monospace shortcut hint. */
export function Kbd(props: HTMLAttributes<HTMLElement>): React.JSX.Element {
  return <kbd {...props} className={styles.kbd} />;
}
