// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { useId, useState } from 'react';
import type { ReactNode } from 'react';
import styles from './ObjectTree.module.css';

/**
 * A folder row of the object tree (Tables, Views, Favourites…): chevron,
 * folder icon, label and count. Its rows mount only while it is open and
 * arrive with a short fade, so collapsing a section of thousands of objects
 * costs nothing and nothing animates layout.
 */
export function TreeSection({
  label,
  count,
  defaultOpen = false,
  forceOpen = false,
  onOpenChange,
  children,
}: {
  label: string;
  /** Child count; omitted while the children are still unknown. */
  count?: number | undefined;
  defaultOpen?: boolean;
  forceOpen?: boolean;
  /** Called with the user-toggled state (lazy loading on expand). */
  onOpenChange?: (open: boolean) => void;
  children: ReactNode;
}): React.JSX.Element {
  const [open, setOpen] = useState(defaultOpen);
  const [touched, setTouched] = useState(false);
  const id = useId();
  const expanded = open || forceOpen;
  return (
    <section className={styles.section}>
      <button
        type="button"
        className={styles.folder}
        aria-expanded={expanded}
        aria-controls={id}
        onClick={() => {
          setOpen(!open);
          setTouched(true);
          onOpenChange?.(!open);
        }}
      >
        <span className={styles.chevron} aria-hidden="true">
          {<Icon name={'chevronRight'} />}
        </span>
        <span className={styles.folderIcon} aria-hidden="true">
          {<Icon name={'folder'} />}
        </span>
        <span className={styles.folderLabel}>{label}</span>
        {count !== undefined && (
          <span className={styles.count}>{count.toLocaleString('en-US')}</span>
        )}
      </button>
      {expanded && (
        <div id={id} className={styles.rows} data-arrive={touched || undefined}>
          {children}
        </div>
      )}
    </section>
  );
}
