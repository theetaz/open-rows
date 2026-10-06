// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import styles from './Navigation.module.css';

/** Object selection row with a name and optional estimated row count. */
export function TreeItem({
  label,
  count,
  selected,
  onSelect,
  children,
}: {
  label: string;
  count?: string;
  selected: boolean;
  onSelect: () => void;
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <button
      type="button"
      aria-label={count ? `${label} ${count}` : label}
      className={styles.treeItem}
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span aria-hidden="true">▦</span>
      <span>{children ?? label}</span>
      <small>{count}</small>
    </button>
  );
}
