// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import styles from './Sheet.module.css';
import { openSheet, sheetKey } from './sheetFocus';

/** Native modal dialog: focus containment, Escape dismissal and focus restoration. */
export function Sheet({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}): React.JSX.Element {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog) return openSheet(dialog);
  }, []);
  return (
    <div
      className={styles.overlay}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <dialog
        ref={ref}
        className={styles.sheet}
        aria-label={title}
        aria-modal="true"
        onKeyDown={(event) => {
          sheetKey(event, onClose);
        }}
        onCancel={(event) => {
          event.preventDefault();
          onClose();
        }}
        onClick={(event) => {
          if (event.target === event.currentTarget) onClose();
        }}
      >
        <div className={styles.body}>
          <header>
            <h2>{title}</h2>
            <button type="button" aria-label="Close sheet" onClick={onClose}>
              ×
            </button>
          </header>
          {children}
        </div>
      </dialog>
    </div>
  );
}
