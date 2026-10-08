// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';

const item = '[role="treeitem"]';

/** Rendered rows in document (visual) order; collapsed groups are absent. */
const rows = (tree: HTMLElement): HTMLElement[] => [
  ...tree.querySelectorAll<HTMLElement>(item),
];

function destination(
  tree: HTMLElement,
  current: HTMLElement,
  key: string,
): HTMLElement | null {
  const all = rows(tree);
  const index = all.indexOf(current);
  switch (key) {
    case 'ArrowDown':
      return all[index + 1] ?? null;
    case 'ArrowUp':
      return index > 0 ? (all[index - 1] ?? null) : null;
    case 'Home':
      return all[0] ?? null;
    case 'End':
      return all.at(-1) ?? null;
    case 'ArrowRight':
      return current.getAttribute('aria-expanded') === 'true'
        ? current.querySelector<HTMLElement>(item)
        : null;
    case 'ArrowLeft':
      return current.parentElement?.closest<HTMLElement>(item) ?? null;
    default:
      return null;
  }
}

/**
 * ARIA tree navigation on the tree element: ↑/↓/Home/End move between
 * rendered rows, → enters an expanded row's group and ← returns to the
 * parent row. Rows handle expand/collapse first and mark the event handled.
 */
export function moveTreeFocus(event: KeyboardEvent<HTMLElement>): void {
  const current = event.target;
  if (event.isDefaultPrevented() || !(current instanceof HTMLElement)) return;
  if (current.getAttribute('role') !== 'treeitem') return;
  const next = destination(event.currentTarget, current, event.key);
  if (!next) return;
  event.preventDefault();
  next.focus();
}
