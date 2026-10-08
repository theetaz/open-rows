// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';

const ROW = '[role="treeitem"]';

/** Rows of a section that can take focus: none while its disclosure is collapsed. */
function openRows(section: Element): HTMLElement[] {
  return [...section.querySelectorAll<HTMLElement>(ROW)].filter(
    (row) => !row.closest('[inert]'),
  );
}

/** Sections the sidebar search has not hidden, in visual order. */
function visibleSections(scope: HTMLElement): HTMLElement[] {
  return [...scope.querySelectorAll<HTMLElement>('section')].filter(
    (section) => !section.closest('[hidden]'),
  );
}

/**
 * ↓ on a section's last row (↑ on its first) carries focus into the next
 * (previous) section: its first (last) row when open, else its disclosure button.
 */
export function moveAcrossSections(event: KeyboardEvent<HTMLElement>): void {
  const row = event.target;
  const down = event.key === 'ArrowDown';
  if (event.isDefaultPrevented() || !(row instanceof HTMLElement)) return;
  if (!down && event.key !== 'ArrowUp') return;
  if (row.getAttribute('role') !== 'treeitem') return;
  const section = row.closest('section');
  if (!section) return;
  const rows = openRows(section);
  if (row !== (down ? rows.at(-1) : rows[0])) return;
  const sections = visibleSections(event.currentTarget);
  const index = sections.indexOf(section);
  const next = index < 0 ? undefined : sections[index + (down ? 1 : -1)];
  if (!next) return;
  const nextRows = openRows(next);
  const target =
    (down ? nextRows[0] : nextRows.at(-1)) ??
    next.querySelector<HTMLElement>('button');
  if (!target) return;
  event.preventDefault();
  target.focus();
}
