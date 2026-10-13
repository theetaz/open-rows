// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';

/** Webview focus follows its native window without installing an IPC listener. */
export function useWindowActive(): void {
  useEffect(() => {
    const root = document.documentElement;
    const previous = root.dataset.windowActive;
    const focus = (): void => {
      root.dataset.windowActive = 'true';
    };
    const blur = (): void => {
      root.dataset.windowActive = 'false';
    };
    root.dataset.windowActive = String(document.hasFocus());
    window.addEventListener('focus', focus);
    window.addEventListener('blur', blur);
    return () => {
      window.removeEventListener('focus', focus);
      window.removeEventListener('blur', blur);
      if (previous === undefined) delete root.dataset.windowActive;
      else root.dataset.windowActive = previous;
    };
  }, []);
}
