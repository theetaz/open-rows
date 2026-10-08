// SPDX-License-Identifier: Apache-2.0
import { useCallback } from 'react';
import type { RefObject } from 'react';

type Offset = { x: number; y: number };

/** Current `translate` offset of a dragged sheet, zero before the first drag. */
function offsetOf(dialog: HTMLElement): Offset {
  const [x = '0', y = '0'] = dialog.style.translate.split(' ');
  return { x: Number.parseFloat(x) || 0, y: Number.parseFloat(y) || 0 };
}

/** Keep `value` between 0 and `max` (or 0 when the sheet is larger than the window). */
function clamp(value: number, max: number): number {
  return Math.min(Math.max(value, 0), Math.max(max, 0));
}

/**
 * Drag a centred sheet by its title row. The sheet stays fully inside the
 * window; each sheet opens centred again because the offset lives on the node.
 */
export function useSheetDrag(
  ref: RefObject<HTMLDialogElement | null>,
): (event: React.PointerEvent<HTMLElement>) => void {
  return useCallback(
    (event) => {
      const dialog = ref.current;
      if (!dialog || event.button !== 0) return;
      event.preventDefault();
      const rect = dialog.getBoundingClientRect();
      const start = { x: event.clientX, y: event.clientY };
      const base = offsetOf(dialog);
      const move = (next: PointerEvent): void => {
        const left = clamp(
          rect.left + next.clientX - start.x,
          window.innerWidth - rect.width,
        );
        const top = clamp(
          rect.top + next.clientY - start.y,
          window.innerHeight - rect.height,
        );
        const x = String(base.x + left - rect.left);
        const y = String(base.y + top - rect.top);
        dialog.style.translate = `${x}px ${y}px`;
      };
      const end = (): void => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', end);
        window.removeEventListener('pointercancel', end);
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', end);
      window.addEventListener('pointercancel', end);
    },
    [ref],
  );
}
