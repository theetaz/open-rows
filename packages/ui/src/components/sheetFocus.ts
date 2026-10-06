// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';

/** Contain keyboard focus even on WebKit versions without HTML dialog support. */
export function sheetKey(
  event: KeyboardEvent<HTMLDialogElement>,
  close: () => void,
): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    close();
    return;
  }
  if (event.key !== 'Tab') return;
  const controls = Array.from(
    event.currentTarget.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input, textarea, select, [tabindex="0"]',
    ),
  );
  const first = controls[0];
  const last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  }
  if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
/** Inert siblings up to the root; restore pre-existing inert state on dismissal. */
function lockBackground(dialog: HTMLElement): () => void {
  const unlocked: Element[] = [];
  let branch: Element = dialog;
  while (branch.parentElement) {
    for (const sibling of branch.parentElement.children) {
      if (sibling !== branch && !sibling.hasAttribute('inert')) {
        sibling.setAttribute('inert', '');
        unlocked.push(sibling);
      }
    }
    branch = branch.parentElement;
    if (branch === document.body) break;
  }
  return () => {
    for (const sibling of unlocked) sibling.removeAttribute('inert');
  };
}
/** Open a modal with an equivalent accessible fallback for macOS 12.0–12.2. */
export function openSheet(dialog: HTMLDialogElement): () => void {
  const previous = document.activeElement;
  const unlock = lockBackground(dialog);
  if (typeof dialog.showModal === 'function') dialog.showModal();
  else dialog.setAttribute('open', '');
  dialog.querySelector<HTMLElement>('button')?.focus();
  return () => {
    if (typeof dialog.close === 'function') dialog.close();
    else dialog.removeAttribute('open');
    unlock();
    if (previous instanceof HTMLElement && previous.isConnected)
      previous.focus();
  };
}
