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
}: {
  label: string;
  count: number;
  defaultOpen?: boolean;
  children: ReactNode;
  forceOpen?: boolean;
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
        <span className={styles.count}>{count}</span>
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
