// SPDX-License-Identifier: Apache-2.0
import type { PointerEvent } from 'react';

/** Pointer travel before a press on a tab becomes a drag. */
const THRESHOLD_PX = 5;

/** Where a tab dropped at `x` lands among the other tabs of its strip. */
function dropIndex(
  centres: readonly number[],
  from: number,
  x: number,
): number {
  const others = centres.filter((_, index) => index !== from);
  const after = others.findIndex((centre) => x < centre);
  return after < 0 ? others.length : after;
}

/** Swallow the click that ends a drag, so dropping does not select. */
function swallowNextClick(): void {
  const swallow = (event: Event): void => {
    event.stopPropagation();
    event.preventDefault();
  };
  window.addEventListener('click', swallow, { capture: true, once: true });
  window.setTimeout(() => {
    window.removeEventListener('click', swallow, { capture: true });
  });
}

/**
 * Start tracking a primary-button press on a tab; dragging it past the
 * threshold and releasing moves it to the drop position via `onReorder`.
 */
export function startTabDrag(
  event: PointerEvent<HTMLElement>,
  id: string,
  onReorder: (id: string, index: number) => void,
): void {
  if (event.button !== 0) return;
  const tab = event.currentTarget;
  const strip = tab.parentElement;
  if (!strip) return;
  const startX = event.clientX;
  let dragging = false;
  const move = (next: globalThis.PointerEvent): void => {
    if (!dragging && Math.abs(next.clientX - startX) < THRESHOLD_PX) return;
    dragging = true;
    tab.dataset.dragging = 'true';
    tab.style.translate = `${String(next.clientX - startX)}px`;
  };
  const end = (last: globalThis.PointerEvent): void => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', end);
    delete tab.dataset.dragging;
    tab.style.translate = '';
    if (!dragging) return;
    swallowNextClick();
    const tabs = [...strip.children];
    const centres = tabs.map((item) => {
      const box = item.getBoundingClientRect();
      return box.left + box.width / 2;
    });
    onReorder(id, dropIndex(centres, tabs.indexOf(tab), last.clientX));
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', end);
}
