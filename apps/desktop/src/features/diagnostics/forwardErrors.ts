// SPDX-License-Identifier: Apache-2.0
import type { UiReport } from '@openrows/ipc';

const errorName = (value: unknown): string =>
  value instanceof Error ? value.name : 'Error';

const reasonText = (value: unknown): string =>
  value instanceof Error ? value.message : String(value);

/**
 * Forward uncaught errors and unhandled promise rejections of `target` to
 * `report` (docs/14 §6). Returns the function that removes the listeners.
 */
export function forwardErrors(
  target: Window,
  report: (report: UiReport) => void,
): () => void {
  const onError = (event: ErrorEvent): void => {
    report({
      kind: 'error',
      name: errorName(event.error),
      message: event.message || null,
      durationMs: null,
    });
  };
  const onRejection = (event: PromiseRejectionEvent): void => {
    report({
      kind: 'unhandledRejection',
      name: errorName(event.reason),
      message: reasonText(event.reason),
      durationMs: null,
    });
  };
  target.addEventListener('error', onError);
  target.addEventListener('unhandledrejection', onRejection);
  return () => {
    target.removeEventListener('error', onError);
    target.removeEventListener('unhandledrejection', onRejection);
  };
}
