// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import styles from './Sheet.module.css';
import { openSheet, sheetKey } from './sheetFocus';

/**
 * macOS-style sheet: compact centred title, 12 px panel corners, focus
 * containment, Escape cancels, Return runs the default (primary) action and
 * focus returns to the opener. There is no close button, as in AppKit.
 */
export function Sheet({
  title,
  onClose,
  children,
  size = 'form',
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  /** `alert` is the narrow confirmation width. */
  size?: 'form' | 'alert';
}): React.JSX.Element {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog) return openSheet(dialog);
  }, []);
  return (
    <div className={styles.overlay}>
      <dialog
        ref={ref}
        className={styles.sheet}
        data-size={size}
        aria-label={title}
        aria-modal="true"
        onKeyDown={(event) => {
          sheetKey(event, onClose);
        }}
        onCancel={(event) => {
          event.preventDefault();
          onClose();
        }}
      >
        <div className={styles.body}>
          <h2>{title}</h2>
          {children}
        </div>
      </dialog>
    </div>
  );
}

/** Push buttons, right-aligned (Cancel then the default action); `leading` sits left. */
export function SheetActions({
  leading,
  children,
}: {
  leading?: ReactNode;
  children: ReactNode;
}): React.JSX.Element {
  return (
    <footer className={styles.actions}>
      {leading}
      <span className={styles.spacer} />
      {children}
    </footer>
  );
}
