// SPDX-License-Identifier: Apache-2.0
import { useId, useState } from 'react';
import type { ReactNode } from 'react';
import styles from './Navigation.module.css';

/** Animated object-tree section with native button keyboard semantics. */
export function DisclosureSection({
  label,
  count,
  defaultOpen = false,
  children,
  forceOpen = false,
  onOpenChange,
}: {
  label: string;
  /** Child count; omitted while the children are still unknown. */
  count?: number;
  defaultOpen?: boolean;
  children: ReactNode;
  forceOpen?: boolean;
  /** Called with the user-toggled state (lazy loading on expand). */
  onOpenChange?: (open: boolean) => void;
}): React.JSX.Element {
  const [open, setOpen] = useState(defaultOpen);
  const id = useId();
  const expanded = open || forceOpen;
  return (
    <section className={styles.section}>
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={id}
        onClick={() => {
          setOpen(!open);
          onOpenChange?.(!open);
        }}
      >
        <span
          className={styles.chevron}
          data-open={expanded}
          aria-hidden="true"
        >
          ›
        </span>
        {label}
        {count !== undefined && <span className={styles.count}>{count}</span>}
      </button>
      <div
        id={id}
        className={styles.disclosure}
        data-open={expanded}
        inert={!expanded}
        aria-hidden={!expanded}
      >
        <div>{children}</div>
      </div>
    </section>
  );
}
