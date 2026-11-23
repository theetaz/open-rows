// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import styles from './Sheet.module.css';
import { useSheetDrag } from './sheetDrag';
import { openSheet, sheetKey } from './sheetFocus';

/**
 * macOS-style sheet: centred in the window and moved by dragging its sticky
 * title row; nearly solid glass, 16 px corners, focus containment,
 * Escape cancels, Return runs the default (primary) action and focus returns
 * to the opener. There is no close button, as in AppKit.
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
  /** `alert` is the narrow confirmation width; `editor` (880 px) fits a SQL editor or a form with a side column. */
  size?: 'form' | 'alert' | 'appearance' | 'editor';
}): React.JSX.Element {
  const ref = useRef<HTMLDialogElement>(null);
  const startDrag = useSheetDrag(ref);
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
          if (
            event.target instanceof Element &&
            event.target.closest('dialog') !== event.currentTarget
          )
            return;
          sheetKey(event, onClose);
        }}
        onScroll={(event) => {
          event.currentTarget.toggleAttribute(
            'data-scrolled',
            event.currentTarget.scrollTop > 0,
          );
        }}
        onCancel={(event) => {
          event.preventDefault();
          onClose();
        }}
      >
        <div className={styles.body}>
          <h2 className={styles.title} onPointerDown={startDrag}>
            {title}
          </h2>
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
