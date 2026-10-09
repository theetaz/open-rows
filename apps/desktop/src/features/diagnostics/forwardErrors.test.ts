// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { UiReport } from '@openrows/ipc';
import { forwardErrors } from './forwardErrors';

it('forwards uncaught errors and unhandled rejections until removed', () => {
  const reports: UiReport[] = [];
  const stop = forwardErrors(window, (report) => reports.push(report));
  window.dispatchEvent(
    new ErrorEvent('error', {
      message: 'boom',
      error: new TypeError('boom'),
    }),
  );
  window.dispatchEvent(
    Object.assign(new Event('unhandledrejection'), {
      reason: new Error('nope'),
    }),
  );
  expect(reports).toEqual([
    { kind: 'error', name: 'TypeError', message: 'boom', durationMs: null },
    {
      kind: 'unhandledRejection',
      name: 'Error',
      message: 'nope',
      durationMs: null,
    },
  ]);
  stop();
  window.dispatchEvent(new ErrorEvent('error', { message: 'later' }));
  expect(reports).toHaveLength(2);
});
