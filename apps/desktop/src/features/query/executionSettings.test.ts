// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { defaultExecution, parseExecution } from './executionSettings';

it('validates every execution field and keeps defaults otherwise', () => {
  expect(parseExecution(null)).toEqual(defaultExecution);
  expect(
    parseExecution({
      runSelection: false,
      autoLimit: 500,
      confirmMultiple: true,
      showNotices: false,
    }),
  ).toEqual({
    runSelection: false,
    autoLimit: 500,
    confirmMultiple: true,
    showNotices: false,
  });
  expect(
    parseExecution({ runSelection: 'no', autoLimit: 7, confirmMultiple: 1 }),
  ).toEqual(defaultExecution);
});
