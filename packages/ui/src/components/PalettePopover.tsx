// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { openSheet, sheetKey } from './sheetFocus';
import { useReducedMotion } from './useReducedMotion';
import { material } from '../tokens/material';
import styles from './Palette.module.css';
/** Modal glass popover retaining focus and its DOM through the closing animation. */
export function PalettePopover(props: {
  title: string;
  onClose: () => void;
  onChoose: (id: string) => void;
  children: (dismiss: (id?: string) => void) => ReactNode;
}): React.JSX.Element {
  const ref = useRef<HTMLDialogElement>(null);
  const [pending, setPending] = useState<string | undefined>(undefined);
  const { onClose, onChoose } = props;
  const [closing, setClosing] = useState(false);
  const reduced = useReducedMotion();
  useEffect(() => {
    if (!ref.current) return;
    const release = openSheet(ref.current);
    ref.current.querySelector('input')?.focus();
    return release;
  }, []);
  useEffect(() => {
    if (!closing) return;
    const finish = (): void => {
      if (pending === undefined) onClose();
      else onChoose(pending);
    };
    const timer = window.setTimeout(
      finish,
      reduced ? 0 : parseFloat(material['motion-popover']),
    );
    return () => {
      window.clearTimeout(timer);
    };
  }, [closing, reduced, onClose, onChoose, pending]);
  const dismiss = (id?: string): void => {
    if (closing) return;
    setPending(id);
    setClosing(true);
  };
  return (
    <div
      className={styles.overlay}
      onClick={(event) => {
        if (event.target === event.currentTarget) dismiss();
      }}
    >
      <dialog
        ref={ref}
        aria-label={props.title}
        aria-modal="true"
        className={styles.popover}
        data-state={closing ? 'closing' : 'open'}
        data-motion={reduced ? 'reduced' : 'full'}
        onKeyDown={(event) => {
          if (!event.nativeEvent.isComposing) sheetKey(event, dismiss);
        }}
        onCancel={(event) => {
          event.preventDefault();
          dismiss();
        }}
        onClick={(event) => {
          if (event.target === event.currentTarget) dismiss();
        }}
      >
        <header>
          <h2>{props.title}</h2>
          <button
            type="button"
            aria-label="Close palette"
            onClick={() => {
              dismiss();
            }}
          >
            ×
          </button>
        </header>
        {props.children(dismiss)}
      </dialog>
    </div>
  );
}
