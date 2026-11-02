// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useState } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { currentWindowLabel } from '@openrows/ipc';
import { scheduleHostPrewarm } from './hostPrewarm';

/**
 * Notify the native shell once React has committed the first render; each
 * workspace window reveals itself, so the signal targets this window.
 *
 * The window starts hidden, and WebKit does not run animation frames for
 * hidden views, so the signal is sent after commit rather than from
 * `requestAnimationFrame`. The DOM is complete at this point, so showing
 * the window cannot flash an empty page. A layout effect sends it before the
 * passive effects of the whole tree (subscriptions, IPC reads), which run
 * child-first and so would otherwise all precede this root's (M0-09).
 */
export function useWindowReady(): boolean {
  const [startupError, setStartupError] = useState(false);
  useLayoutEffect(() => {
    let disposed = false;
    let cancel: (() => void) | undefined;
    performance.mark('openrows:first-paint-ready');
    void emitTo(currentWindowLabel(), 'openrows:first-paint-ready')
      .then(() => {
        if (!disposed) cancel = scheduleHostPrewarm();
      })
      .catch(() => {
        setStartupError(true);
      });
    return () => {
      disposed = true;
      cancel?.();
    };
  }, []);

  return startupError;
}
