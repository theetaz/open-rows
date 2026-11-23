// SPDX-License-Identifier: Apache-2.0
import { useCallback } from 'react';
import type { RefObject } from 'react';

type Offset = { x: number; y: number };

/** Current `translate` offset of a dragged sheet, zero before the first drag. */
function offsetOf(dialog: HTMLElement): Offset {
  const [x = '0', y = '0'] = dialog.style.translate.split(' ');
  return { x: Number.parseFloat(x) || 0, y: Number.parseFloat(y) || 0 };
}

/** The traffic lights' rectangle (CSS px): a sheet left of `x` stays below `y`. */
const TRAFFIC_LIGHTS = { x: 84, y: 48 };

/** Keep `value` between `min` and `max` (or `min` when the sheet is larger than the room). */
function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(max, min));
}

/**
 * Where a sheet of `size` may sit with its top-left at `wanted`: fully inside
 * the window, and never over the traffic lights.
 */
function clampSheet(
  wanted: Offset,
  size: { width: number; height: number },
): Offset {
  const x = clamp(wanted.x, 0, window.innerWidth - size.width);
  const top = x < TRAFFIC_LIGHTS.x ? TRAFFIC_LIGHTS.y : 0;
  const y = clamp(wanted.y, top, window.innerHeight - size.height);
  return { x, y };
}

/**
 * Drag a centred sheet by its title row. The sheet stays fully inside the
 * window and clear of the traffic lights; each sheet opens centred again because the offset lives on the node.
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
        const to = clampSheet(
          {
            x: rect.left + next.clientX - start.x,
            y: rect.top + next.clientY - start.y,
          },
          rect,
        );
        const x = String(base.x + to.x - rect.left);
        const y = String(base.y + to.y - rect.top);
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
