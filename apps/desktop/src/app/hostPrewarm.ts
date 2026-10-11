// SPDX-License-Identifier: Apache-2.0
import { emitTo } from '@tauri-apps/api/event';
import { reportUiEvent } from '@openrows/ipc';
import { createReporter } from '../features/diagnostics/reporter';

const report = createReporter(reportUiEvent);

/** Two visible frames precede idle work; cancellation also covers the fallback. */
export function scheduleHostPrewarm(): () => void {
  let frame = 0;
  let idle: number | undefined;
  let timer: number | undefined;
  const signal = (): void => {
    void emitTo('main', 'openrows:driver-host-idle').catch(() => {
      report({
        kind: 'error',
        name: 'driverHostPrewarm',
        message: 'Driver host idle signal failed',
        durationMs: null,
      });
    });
  };
  frame = requestAnimationFrame(() => {
    frame = requestAnimationFrame(() => {
      if (typeof window.requestIdleCallback === 'function')
        idle = window.requestIdleCallback(signal);
      else timer = window.setTimeout(signal, 0);
    });
  });
  return () => {
    cancelAnimationFrame(frame);
    if (idle !== undefined) window.cancelIdleCallback(idle);
    if (timer !== undefined) window.clearTimeout(timer);
  };
}
