// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
/** Suspend modal top-layer sheets so native modality cannot make unlock inert. */
export function useSuspendedDialogs(locked: boolean): void {
  useEffect(() => {
    if (!locked) return;
    const paused = new Set<HTMLDialogElement>();
    const suspend = (): void => {
      document
        .querySelectorAll<HTMLDialogElement>('dialog[open]')
        .forEach((dialog) => {
          paused.add(dialog);
          if (typeof dialog.close === 'function') dialog.close();
          else dialog.removeAttribute('open');
        });
      document
        .querySelector('[aria-label="OpenRows locked"]')
        ?.removeAttribute('inert');
    };
    suspend();
    const observer = new MutationObserver(suspend);
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['open', 'inert'],
    });
    return () => {
      observer.disconnect();
      paused.forEach((dialog) => {
        if (!dialog.isConnected) return;
        if (typeof dialog.showModal === 'function') dialog.showModal();
        else dialog.setAttribute('open', '');
      });
    };
  }, [locked]);
}
