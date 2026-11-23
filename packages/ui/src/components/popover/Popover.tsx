// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { material } from '../../tokens/material';
import { belowAnchor, placeFloating } from '../menu/placement';
import { useReducedMotion } from '../useReducedMotion';
import styles from './Popover.module.css';

export type PopoverProps = {
  /** The control it opens from; the popover sits 6 px under it. */
  anchor: Element;
  /** Accessible name of the dialog. */
  label: string;
  /** Called after the closing fade; unmount the popover then. */
  onClose: () => void;
  /** Shift left of the anchor's leading edge (e.g. to line up a search icon). */
  dx?: number;
  /** Centre under the anchor instead of aligning leading edges. */
  center?: boolean;
  /** Clicks on elements matching this selector do not close it. */
  keepOn?: string;
  className?: string;
  children: ReactNode;
};

/** Where the popover goes under its anchor. */
function place(panel: HTMLElement, props: PopoverProps): void {
  const box = props.anchor.getBoundingClientRect();
  const at = belowAnchor(box, { gap: 6 });
  const x = props.center
    ? box.left + box.width / 2 - panel.offsetWidth / 2
    : at.x - (props.dx ?? 0);
  placeFloating(panel, { ...at, x });
}

/** Outside clicks and Escape fade the popover out, then close it. */
function useDismiss(
  panel: React.RefObject<HTMLDivElement | null>,
  props: PopoverProps,
): { closing: boolean } {
  const reduced = useReducedMotion();
  const [closing, setClosing] = useState(false);
  const { anchor, keepOn, onClose } = props;
  useEffect(() => {
    let timer = 0;
    const close = (): void => {
      if (reduced) {
        onClose();
        return;
      }
      setClosing(true);
      timer = window.setTimeout(
        onClose,
        parseFloat(material['motion-popover-out']),
      );
    };
    const down = (event: PointerEvent): void => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (panel.current?.contains(target) || anchor.contains(target)) return;
      if (target.closest('[role="menu"]') || (keepOn && target.closest(keepOn)))
        return;
      close();
    };
    const key = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      event.preventDefault();
      event.stopPropagation();
      if (anchor instanceof HTMLElement) anchor.focus({ preventScroll: true });
      close();
    };
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('keydown', key, true);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('pointerdown', down, true);
      document.removeEventListener('keydown', key, true);
    };
  }, [panel, anchor, keepOn, onClose, reduced]);
  return { closing };
}

/**
 * A floating panel under a control: float material, 12 px corners, a short
 * rise in, a quicker fade out, clamped inside the window. Escape and outside
 * clicks close it. Mount it while open; the anchor should carry
 * `aria-expanded`. For a searchable list use `Picker`.
 *
 * ```tsx
 * {open && anchor && (
 *   <Popover anchor={anchor} label="Columns" onClose={() => setOpen(false)}>…</Popover>
 * )}
 * ```
 */
export function Popover(props: PopoverProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const { closing } = useDismiss(ref, props);
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  useLayoutEffect(() => {
    if (ref.current) place(ref.current, latest.current);
  }, [props.anchor]);
  return createPortal(
    <div
      ref={ref}
      role="dialog"
      aria-label={props.label}
      className={`${styles.popover ?? ''} ${props.className ?? ''}`}
      data-state={closing ? 'closing' : 'open'}
    >
      {props.children}
    </div>,
    document.body,
  );
}
