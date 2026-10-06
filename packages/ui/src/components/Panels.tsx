// SPDX-License-Identifier: Apache-2.0
import type { HTMLAttributes } from 'react';
import styles from './Panels.module.css';

/** Floating material surface; solid under reduced transparency. */
export function GlassPanel({
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement>): React.JSX.Element {
  return <div {...props} className={`${styles.panel ?? ''} ${className}`} />;
}
/** Labelled glass capsule containing related toolbar controls. */
export function ToolbarGroup({
  className = '',
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  'aria-label': string;
}): React.JSX.Element {
  return (
    <div
      {...props}
      role="group"
      className={`${styles.toolbar ?? ''} ${className}`}
    />
  );
}
