// SPDX-License-Identifier: Apache-2.0
import type { KeyboardEvent } from 'react';

const FOCUSABLE =
  'button:not(:disabled), input:not(:disabled), textarea, select, [tabindex="0"]';
const FIELD = 'input:not(:disabled):not([readonly]), textarea, select';
const DEFAULT_ACTION = 'button[data-variant="primary"]:not(:disabled)';

/** Return outside forms and buttons runs the sheet's primary push button. */
function runDefault(event: KeyboardEvent<HTMLDialogElement>): void {
  if (event.nativeEvent.isComposing || event.metaKey || event.shiftKey) return;
  const target = event.target as HTMLElement;
  if (target.closest('button, textarea, select, a, form')) return;
  const action =
    event.currentTarget.querySelector<HTMLButtonElement>(DEFAULT_ACTION);
  if (!action) return;
  event.preventDefault();
  action.click();
}

function containTab(event: KeyboardEvent<HTMLDialogElement>): void {
  const controls = Array.from(
    event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE),
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

/** Escape cancels, Return runs the default action, Tab stays inside (old WebKit too). */
export function sheetKey(
  event: KeyboardEvent<HTMLDialogElement>,
  close: () => void,
): void {
  if (event.key === 'Escape') {
    event.preventDefault();
    close();
  } else if (event.key === 'Enter') runDefault(event);
  else if (event.key === 'Tab') containTab(event);
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
/** Like AppKit: the first text field, else the default button, else the first control. */
function initialFocus(dialog: HTMLElement): HTMLElement | null {
  return (
    dialog.querySelector<HTMLElement>('[autofocus]') ??
    dialog.querySelector<HTMLElement>(FIELD) ??
    dialog.querySelector<HTMLElement>(DEFAULT_ACTION) ??
    dialog.querySelector<HTMLElement>(FOCUSABLE)
  );
}
/** Open a modal with an equivalent accessible fallback for macOS 12.0–12.2. */
export function openSheet(dialog: HTMLDialogElement): () => void {
  const previous = document.activeElement;
  const unlock = lockBackground(dialog);
  if (typeof dialog.showModal === 'function') dialog.showModal();
  else dialog.setAttribute('open', '');
  initialFocus(dialog)?.focus();
  return () => {
    if (typeof dialog.close === 'function') dialog.close();
    else dialog.removeAttribute('open');
    unlock();
    if (previous instanceof HTMLElement && previous.isConnected)
      previous.focus();
  };
}
